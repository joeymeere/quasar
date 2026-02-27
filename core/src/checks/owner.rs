use crate::prelude::*;

pub trait Owner: crate::traits::Owner {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        // SAFETY: Same invariant as CheckOwner — called at parse time only,
        // before any handler mutation.
        if !crate::keys_eq(unsafe { view.owner() }, &Self::OWNER) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}
