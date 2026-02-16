use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Data, DeriveInput, FnArg, Fields, ItemFn, LitInt, Token, Type,
};

#[proc_macro_derive(Accounts)]
pub fn derive_accounts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("Accounts can only be derived for structs with named fields"),
        },
        _ => panic!("Accounts can only be derived for structs"),
    };

    let field_names: Vec<_> = fields.iter().map(|f| &f.ident).collect();

    let field_constructs: Vec<proc_macro2::TokenStream> = fields.iter().map(|f| {
        let name = &f.ident;
        match &f.ty {
            Type::Reference(type_ref) => {
                let base_type = strip_generics(&type_ref.elem);
                if type_ref.mutability.is_some() {
                    quote! { #name: #base_type::from_account_view_mut(#name)? }
                } else {
                    quote! { #name: #base_type::from_account_view(#name)? }
                }
            }
            _ => {
                let base_type = strip_generics(&f.ty);
                quote! { #name: #base_type::from_account_view(#name)? }
            }
        }
    }).collect();

    let expanded = quote! {
        impl<'info> TryFrom<&'info [AccountView]> for #name<'info> {
            type Error = ProgramError;

            #[inline(always)]
            fn try_from(accounts: &'info [AccountView]) -> Result<Self, Self::Error> {
                let [#(#field_names),*] = accounts else {
                    return Err(ProgramError::NotEnoughAccountKeys);
                };

                Ok(Self {
                    #(#field_constructs,)*
                })
            }
        }
    };

    TokenStream::from(expanded)
}

/// Parses: `discriminator = <u8_literal>`
struct InstructionArgs {
    discriminator: LitInt,
}

impl Parse for InstructionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        if ident != "discriminator" {
            return Err(syn::Error::new(ident.span(), "expected `discriminator`"));
        }
        let _: Token![=] = input.parse()?;
        let discriminator: LitInt = input.parse()?;
        Ok(Self { discriminator })
    }
}

#[proc_macro_attribute]
pub fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let mut func = parse_macro_input!(item as ItemFn);
    let discriminator = &args.discriminator;

    // Extract first parameter (ctx: Ctx<T>)
    let first_arg = match func.sig.inputs.first() {
        Some(FnArg::Typed(pt)) => pt.clone(),
        _ => panic!("#[instruction] requires ctx: Ctx<T> as first parameter"),
    };

    let param_name = &first_arg.pat;
    let param_type = &first_arg.ty;

    // Replace first param with context: Context
    *func.sig.inputs.first_mut().unwrap() = syn::parse_quote!(mut context: Context);

    // Prepend: discriminator check + ctx construction
    let stmts = std::mem::take(&mut func.block.stmts);
    func.block.stmts = [
        syn::parse_quote!(
            if context.data.first() != Some(&#discriminator) {
                return Err(ProgramError::InvalidInstructionData);
            }
        ),
        syn::parse_quote!(
            context.data = &context.data[1..];
        ),
        syn::parse_quote!(
            let #param_name: #param_type = Ctx::new(context)?;
        ),
    ]
    .into_iter()
    .chain(stmts)
    .collect();

    quote!(#func).into()
}

#[proc_macro_attribute]
pub fn account(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let discriminator = &args.discriminator;

    let field_types: Vec<_> = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields.named.iter().map(|f| &f.ty).collect(),
            _ => panic!("#[account] can only be used on structs with named fields"),
        },
        _ => panic!("#[account] can only be used on structs"),
    };

    quote! {
        #[repr(C)]
        #[derive(::wincode::SchemaRead, ::wincode::SchemaWrite)]
        #input

        impl Discriminator for #name {
            const DISCRIMINATOR: u8 = #discriminator;
        }

        impl Space for #name {
            const SPACE: usize = 1 #(+ core::mem::size_of::<#field_types>())*;
        }

        impl Owner for #name {
            const OWNER: Address = crate::ID;
        }

        impl QuasarAccount for #name {
            #[inline(always)]
            fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
                ::wincode::deserialize(data).map_err(|_| ProgramError::InvalidAccountData)
            }

            #[inline(always)]
            fn serialize(&self, data: &mut [u8]) -> Result<(), ProgramError> {
                ::wincode::serialize_into(data, self).map_err(|_| ProgramError::InvalidAccountData)
            }
        }

        impl core::ops::Deref for Account<#name> {
            type Target = #name;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.to_account_view().borrow_unchecked().as_ptr().add(1) as *const #name) }
            }
        }

        impl #name {
            #[inline(always)]
            pub fn init(self, account: &mut Initialize<Self>, payer: &AccountView, rent: &Rent) -> Result<(), ProgramError> {
                self.init_signed(account, payer, rent, &[])
            }

            #[inline(always)]
            pub fn init_signed(self, account: &mut Initialize<Self>, payer: &AccountView, rent: &Rent, signers: &[pinocchio::cpi::Signer]) -> Result<(), ProgramError> {
                let lamports = account.to_account_view().lamports();
                let rent_exempt_lamports = rent.get()?.try_minimum_balance(Self::SPACE)?;
                if lamports == 0 {
                    pinocchio_system::instructions::CreateAccount {
                        from: payer,
                        to: account.to_account_view(),
                        lamports: rent_exempt_lamports,
                        space: Self::SPACE as u64,
                        owner: &Self::OWNER
                    }.invoke_signed(signers)?;
                } else {
                    // // todo: handle for assign/allocate
                    // pinocchio_system::instructions::Transfer {
                    //     from: payer,
                    //     to: account.to_account_view(),
                    //     lamports: rent_exempt_lamports,
                    //     space: Self::SPACE as u64,
                    //     owner: &Self::OWNER
                    // }.invoke_signed(signers)?;
                }

                let mut data = account.to_account_view().try_borrow_mut()?;
                data[0] = Self::DISCRIMINATOR;
                self.serialize(&mut data[1..])?;
                Ok(())
            }
        }
    }.into()
}

/// Strips generic arguments from a type path.
/// e.g. `Signer<'info>` -> `Signer`
fn strip_generics(ty: &Type) -> proc_macro2::TokenStream {
    match ty {
        Type::Path(type_path) => {
            let segments: Vec<_> = type_path
                .path
                .segments
                .iter()
                .map(|seg| &seg.ident)
                .collect();
            quote! { #(#segments)::* }
        }
        _ => panic!("Unsupported field type"),
    }
}
