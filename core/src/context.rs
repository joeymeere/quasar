use crate::prelude::*;
use crate::remaining::RemainingAccounts;

/// Raw entrypoint context before parsing.
pub struct Context<'info> {
    pub program_id: &'info [u8; 32],
    pub accounts: &'info [AccountView],
    pub remaining_ptr: *mut u8,
    pub data: &'info [u8],
}

/// Parsed instruction context with typed accounts and PDA bumps.
pub struct Ctx<'info, T: ParseAccounts<'info> + AccountCount> {
    pub accounts: T,
    pub bumps: T::Bumps,
    pub program_id: &'info [u8; 32],
    pub data: &'info [u8],
    remaining_ptr: *mut u8,
    declared: &'info [AccountView],
}

impl<'info, T: ParseAccounts<'info> + AccountCount> Ctx<'info, T> {
    #[inline(always)]
    pub fn new(ctx: Context<'info>) -> Result<Self, ProgramError> {
        let (accounts, bumps) = T::parse(ctx.accounts)?;
        Ok(Self {
            accounts,
            bumps,
            program_id: ctx.program_id,
            data: ctx.data,
            remaining_ptr: ctx.remaining_ptr,
            declared: ctx.accounts,
        })
    }

    /// Access remaining accounts. Zero cost until called.
    #[inline(always)]
    pub fn remaining_accounts(&self) -> RemainingAccounts<'info> {
        let boundary = unsafe { self.data.as_ptr().sub(core::mem::size_of::<u64>()) };
        RemainingAccounts::new(self.remaining_ptr, boundary, self.declared)
    }
}
