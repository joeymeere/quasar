use {
    crate::{
        constants::{TOKEN_2022_BYTES, TOKEN_2022_ID},
        instructions::TokenCpi,
        state::{MintAccountState, TokenAccountState},
        token::{MintParams, TokenParams},
    },
    quasar_lang::{prelude::*, traits::Id},
};

/// Token account view — validates owner is Token-2022 program.
///
/// Also implements `Id`, so `Program<Token2022>` serves as the program account
/// type.
#[repr(transparent)]
pub struct Token2022 {
    __view: AccountView,
}
impl_program_account!(Token2022, TOKEN_2022_ID, TokenAccountState);

impl Id for Token2022 {
    const ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
}

/// Mint account view — validates owner is Token-2022 program.
#[repr(transparent)]
pub struct Mint2022 {
    __view: AccountView,
}
impl_program_account!(Mint2022, TOKEN_2022_ID, MintAccountState);

impl TokenCpi for Program<Token2022> {}

// ---------------------------------------------------------------------------
// AccountInner impls — Token2022 / Mint2022
// ---------------------------------------------------------------------------

impl AccountInner for Token2022 {
    type Params = TokenParams;

    #[inline(always)]
    fn validate(view: &AccountView, params: &Self::Params) -> Result<(), ProgramError> {
        let (mint, authority) = match (&params.mint, &params.authority) {
            (Some(m), Some(a)) => (m, a),
            _ => return Ok(()),
        };
        let token_program = params.token_program.as_ref().unwrap_or(&TOKEN_2022_ID);
        crate::validate::validate_token_account(view, mint, authority, token_program)
    }
}

impl AccountInner for Mint2022 {
    type Params = MintParams;

    #[inline(always)]
    fn validate(view: &AccountView, params: &Self::Params) -> Result<(), ProgramError> {
        let (authority, decimals) = match (&params.authority, params.decimals) {
            (Some(a), Some(d)) => (a, d),
            _ => return Ok(()),
        };
        let token_program = params.token_program.as_ref().unwrap_or(&TOKEN_2022_ID);
        let freeze_authority = params.freeze_authority.as_ref();
        crate::validate::validate_mint(view, authority, decimals, freeze_authority, token_program)
    }
}
