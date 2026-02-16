use core::marker::PhantomData;
use crate::prelude::*;

#[repr(transparent)]
pub struct Account<T: Owner> {
    view: AccountView,
    _marker: PhantomData<T>,
}

impl<T: Owner> Account<T> {
    #[inline(always)]
    pub fn to_account_view(&self) -> &AccountView {
        &self.view
    }

    #[inline(always)]
    pub fn owner(&self) -> &'static Address {
        &T::OWNER
    }

    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        if !view.owned_by(&T::OWNER) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }

    #[inline(always)]
    #[allow(invalid_reference_casting)]
    pub fn from_account_view_mut(view: &AccountView) -> Result<&mut Self, ProgramError> {
        if !view.is_writable() {
            return Err(ProgramError::Immutable);
        }
        if !view.owned_by(&T::OWNER) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(unsafe { &mut *(view as *const AccountView as *mut Self) })
    }
}

impl<T: QuasarAccount + Owner> Account<T> {
    #[inline(always)]
    pub fn get(&self) -> Result<T, ProgramError> {
        let data = self.view.try_borrow()?;
        if data.first() != Some(&T::DISCRIMINATOR) {
            return Err(ProgramError::InvalidAccountData);
        }
        T::deserialize(&data[1..])
    }

    #[inline(always)]
    pub fn set(&mut self, value: &T) -> Result<(), ProgramError> {
        let mut data = self.view.try_borrow_mut()?;
        value.serialize(&mut data[1..])
    }
}
