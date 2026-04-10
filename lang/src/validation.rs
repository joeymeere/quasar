//! Runtime validation helpers for account constraint checks.
//!
//! These functions are called by proc-macro-generated code to validate
//! `has_one` and `address` constraints. Custom errors are supported via
//! the `error` parameter — the macro resolves `@ MyError` expressions
//! at compile time and passes them here.

use {crate::utils::hint::unlikely, solana_address::Address, solana_program_error::ProgramError};

/// Validate that two addresses match (used for `has_one` constraints).
///
/// The `error` parameter supports custom errors via
/// `#[account(has_one = x @ MyError::Unauthorized)]`.
#[inline(always)]
pub fn check_has_one(
    stored: &Address,
    expected: &Address,
    error: ProgramError,
) -> Result<(), ProgramError> {
    if unlikely(!crate::keys_eq(stored, expected)) {
        return Err(error);
    }
    Ok(())
}

/// Validate that an account's address matches an expected value.
///
/// The `error` parameter supports custom errors via
/// `#[account(address = expr @ MyError)]`.
#[inline(always)]
pub fn check_address_match(
    actual: &Address,
    expected: &Address,
    error: ProgramError,
) -> Result<(), ProgramError> {
    if unlikely(!crate::keys_eq(actual, expected)) {
        return Err(error);
    }
    Ok(())
}
