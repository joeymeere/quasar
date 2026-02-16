use crate::prelude::*;

define_account!(pub struct SystemProgram => [checks::Executable, checks::Address]);

impl crate::traits::Program for SystemProgram {
    const ID: Address = Address::new_from_array([0u8; 32]);
}
