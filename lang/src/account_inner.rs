use {solana_account_view::AccountView, solana_program_error::ProgramError};

/// Validation trait for inner types used in `Account<T>` /
/// `InterfaceAccount<T>`.
///
/// SPL types provide specific `Params` structs for namespaced constraints
/// (`token::mint`, `mint::decimals`). User `#[account]` types get `Params =
/// ()`.
pub trait AccountInner {
    type Params: Default;

    #[inline(always)]
    fn validate(view: &AccountView, _params: &Self::Params) -> Result<(), ProgramError> {
        let _ = view;
        Ok(())
    }
}
