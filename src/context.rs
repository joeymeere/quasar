use crate::prelude::*;

/// Raw entrypoint context before parsing.
pub struct Context<'info> {
    pub program_id: &'info [u8; 32],
    pub accounts: &'info [AccountView],
    pub data: &'info [u8],
}

/// Parsed instruction context with typed accounts.
pub struct Ctx<'info, T> {
    pub accounts: T,
    pub program_id: &'info [u8; 32],
    pub data: &'info [u8],
}

impl<'info, T: TryFrom<&'info [AccountView], Error = ProgramError>> Ctx<'info, T> {
    #[inline(always)]
    pub fn new(ctx: Context<'info>) -> Result<Self, ProgramError> {
        let accounts = T::try_from(ctx.accounts)?;
        Ok(Self {
            accounts,
            program_id: ctx.program_id,
            data: ctx.data,
        })
    }
}
