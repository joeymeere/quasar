use core::marker::PhantomData;
use crate::prelude::*;

#[repr(transparent)]
pub struct Initialize<T: QuasarAccount> {
    view: AccountView,
    _marker: PhantomData<T>,
}

impl<T: QuasarAccount> Initialize<T> {
    #[inline(always)]
    pub fn to_account_view(&self) -> &AccountView {
        &self.view
    }

    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }

    #[inline(always)]
    #[allow(invalid_reference_casting)]
    pub fn from_account_view_mut(view: &AccountView) -> Result<&mut Self, ProgramError> {
        if !view.is_writable() {
            return Err(ProgramError::Immutable);
        }
        Ok(unsafe { &mut *(view as *const AccountView as *mut Self) })
    }
}

impl<T: QuasarAccount> core::ops::Deref for Initialize<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.view.borrow_unchecked().as_ptr().add(1) as *const T) }
    }
}

impl<T: QuasarAccount> core::ops::DerefMut for Initialize<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.view.borrow_unchecked_mut().as_mut_ptr().add(1) as *mut T) }
    }
}
