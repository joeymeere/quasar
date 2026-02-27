#![allow(dead_code)]

use std::mem::size_of;

use quasar_core::__internal::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED};
use quasar_core::checks;
use quasar_core::checks::{Address as AddressCheck, Mutable, Owner, Signer};
use quasar_core::cpi::system::SystemProgram;
use quasar_core::traits::AsAccountView;
use solana_address::Address;
use solana_program_error::ProgramError;

// ---------------------------------------------------------------------------
// Test helpers (duplicated from miri.rs)
// ---------------------------------------------------------------------------

struct AccountBuffer {
    inner: Vec<u64>,
}

impl AccountBuffer {
    fn new(data_len: usize) -> Self {
        let byte_len =
            size_of::<RuntimeAccount>() + data_len + MAX_PERMITTED_DATA_INCREASE + size_of::<u64>();
        let u64_count = (byte_len + 7) / 8;
        Self {
            inner: vec![0; u64_count],
        }
    }

    fn raw(&mut self) -> *mut RuntimeAccount {
        self.inner.as_mut_ptr() as *mut RuntimeAccount
    }

    fn init(
        &mut self,
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data_len: u64,
        is_signer: bool,
        is_writable: bool,
    ) {
        let raw = self.raw();
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = is_signer as u8;
            (*raw).is_writable = is_writable as u8;
            (*raw).executable = 0;
            (*raw).resize_delta = 0;
            (*raw).address = Address::new_from_array(address);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = lamports;
            (*raw).data_len = data_len;
        }
    }

    fn init_executable(
        &mut self,
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data_len: u64,
        is_signer: bool,
        is_writable: bool,
    ) {
        let raw = self.raw();
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = is_signer as u8;
            (*raw).is_writable = is_writable as u8;
            (*raw).executable = 1;
            (*raw).resize_delta = 0;
            (*raw).address = Address::new_from_array(address);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = lamports;
            (*raw).data_len = data_len;
        }
    }

    unsafe fn view(&mut self) -> AccountView {
        AccountView::new_unchecked(self.raw())
    }

    fn write_data(&mut self, data: &[u8]) {
        let data_start = size_of::<RuntimeAccount>();
        let dst = unsafe {
            let ptr = (self.inner.as_mut_ptr() as *mut u8).add(data_start);
            std::slice::from_raw_parts_mut(ptr, data.len())
        };
        dst.copy_from_slice(data);
    }
}

// ---------------------------------------------------------------------------
// Owner validation
// ---------------------------------------------------------------------------

// Stub type for owner checks
struct TestOwner;
impl quasar_core::traits::Owner for TestOwner {
    const OWNER: Address = Address::new_from_array([1u8; 32]);
}
impl checks::Owner for TestOwner {}

#[test]
fn owner_check_correct_owner() {
    let mut buf = AccountBuffer::new(32);
    buf.init([0u8; 32], [1u8; 32], 1000, 32, false, false);
    let view = unsafe { buf.view() };
    assert!(TestOwner::check(&view).is_ok());
}

#[test]
fn owner_check_wrong_owner() {
    let mut owner = [1u8; 32];
    owner[31] = 2; // single byte difference
    let mut buf = AccountBuffer::new(32);
    buf.init([0u8; 32], owner, 1000, 32, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        TestOwner::check(&view),
        Err(ProgramError::IllegalOwner)
    ));
}

#[test]
fn owner_check_all_zero_owner() {
    let mut buf = AccountBuffer::new(32);
    buf.init([0u8; 32], [0u8; 32], 1000, 32, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        TestOwner::check(&view),
        Err(ProgramError::IllegalOwner)
    ));
}

// ---------------------------------------------------------------------------
// Signer validation
// ---------------------------------------------------------------------------

struct TestSigner;
impl checks::Signer for TestSigner {}

#[test]
fn signer_check_is_signer() {
    let mut buf = AccountBuffer::new(0);
    buf.init([0u8; 32], [0u8; 32], 0, 0, true, false);
    let view = unsafe { buf.view() };
    assert!(TestSigner::check(&view).is_ok());
}

#[test]
fn signer_check_not_signer() {
    let mut buf = AccountBuffer::new(0);
    buf.init([0u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        TestSigner::check(&view),
        Err(ProgramError::MissingRequiredSignature)
    ));
}

// ---------------------------------------------------------------------------
// Writable validation
// ---------------------------------------------------------------------------

struct TestMutable;
impl checks::Mutable for TestMutable {}

#[test]
fn mutable_check_writable() {
    let mut buf = AccountBuffer::new(0);
    buf.init([0u8; 32], [0u8; 32], 0, 0, false, true);
    let view = unsafe { buf.view() };
    assert!(TestMutable::check(&view).is_ok());
}

#[test]
fn mutable_check_not_writable() {
    let mut buf = AccountBuffer::new(0);
    buf.init([0u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        TestMutable::check(&view),
        Err(ProgramError::Immutable)
    ));
}

// ---------------------------------------------------------------------------
// Address validation
// ---------------------------------------------------------------------------

struct TestAddress;
impl quasar_core::traits::Program for TestAddress {
    const ID: Address = Address::new_from_array([42u8; 32]);
}
impl AddressCheck for TestAddress {}

#[test]
fn address_check_matching() {
    let mut buf = AccountBuffer::new(0);
    buf.init([42u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(<TestAddress as AddressCheck>::check(&view).is_ok());
}

#[test]
fn address_check_non_matching() {
    let mut buf = AccountBuffer::new(0);
    buf.init([43u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        <TestAddress as AddressCheck>::check(&view),
        Err(ProgramError::IncorrectProgramId)
    ));
}

// ---------------------------------------------------------------------------
// keys_eq adversarial tests
// ---------------------------------------------------------------------------

#[test]
fn keys_eq_identical() {
    let a = Address::new_from_array([0xAA; 32]);
    let b = Address::new_from_array([0xAA; 32]);
    assert!(quasar_core::keys_eq(&a, &b));
}

#[test]
fn keys_eq_differ_first_byte() {
    let a = Address::new_from_array([0x00; 32]);
    let mut b_bytes = [0x00u8; 32];
    b_bytes[0] = 0x01;
    let b = Address::new_from_array(b_bytes);
    assert!(!quasar_core::keys_eq(&a, &b));
}

#[test]
fn keys_eq_differ_last_byte() {
    let a = Address::new_from_array([0x00; 32]);
    let mut b_bytes = [0x00u8; 32];
    b_bytes[31] = 0x01;
    let b = Address::new_from_array(b_bytes);
    assert!(!quasar_core::keys_eq(&a, &b));
}

#[test]
fn keys_eq_differ_middle_byte() {
    let a = Address::new_from_array([0x00; 32]);
    let mut b_bytes = [0x00u8; 32];
    b_bytes[15] = 0x01;
    let b = Address::new_from_array(b_bytes);
    assert!(!quasar_core::keys_eq(&a, &b));
}

#[test]
fn keys_eq_all_zeros() {
    let a = Address::new_from_array([0x00; 32]);
    let b = Address::new_from_array([0x00; 32]);
    assert!(quasar_core::keys_eq(&a, &b));
}

#[test]
fn keys_eq_all_ones() {
    let a = Address::new_from_array([0xFF; 32]);
    let b = Address::new_from_array([0xFF; 32]);
    assert!(quasar_core::keys_eq(&a, &b));
}

#[test]
fn keys_eq_single_bit_byte_0() {
    let a = Address::new_from_array([0x00; 32]);
    let mut b_bytes = [0x00u8; 32];
    b_bytes[0] = 0x01; // bit 0 of byte 0
    let b = Address::new_from_array(b_bytes);
    assert!(!quasar_core::keys_eq(&a, &b));
}

#[test]
fn keys_eq_single_bit_byte_31() {
    let a = Address::new_from_array([0x00; 32]);
    let mut b_bytes = [0x00u8; 32];
    b_bytes[31] = 0x80; // bit 7 of byte 31
    let b = Address::new_from_array(b_bytes);
    assert!(!quasar_core::keys_eq(&a, &b));
}

// ---------------------------------------------------------------------------
// define_account! macro tests
// ---------------------------------------------------------------------------

quasar_core::define_account!(pub struct TestSignerAccount => [checks::Signer]);

#[test]
fn define_account_signer_ok() {
    let mut buf = AccountBuffer::new(0);
    buf.init([0u8; 32], [0u8; 32], 0, 0, true, false);
    let view = unsafe { buf.view() };
    assert!(TestSignerAccount::from_account_view(&view).is_ok());
}

#[test]
fn define_account_signer_fails() {
    let mut buf = AccountBuffer::new(0);
    buf.init([0u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        TestSignerAccount::from_account_view(&view),
        Err(ProgramError::MissingRequiredSignature)
    ));
}

#[test]
fn define_account_mut_requires_writable() {
    let mut buf = AccountBuffer::new(0);
    buf.init([0u8; 32], [0u8; 32], 0, 0, true, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        TestSignerAccount::from_account_view_mut(&view),
        Err(ProgramError::Immutable)
    ));
}

#[test]
fn define_account_mut_writable_and_signer_ok() {
    let mut buf = AccountBuffer::new(0);
    buf.init([0u8; 32], [0u8; 32], 0, 0, true, true);
    let view = unsafe { buf.view() };
    assert!(TestSignerAccount::from_account_view_mut(&view).is_ok());
}

// ---------------------------------------------------------------------------
// SystemProgram address check (real program type)
// ---------------------------------------------------------------------------

#[test]
fn system_program_correct_address() {
    let mut buf = AccountBuffer::new(0);
    buf.init_executable([0u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(SystemProgram::from_account_view(&view).is_ok());
}

#[test]
fn system_program_wrong_address() {
    let mut buf = AccountBuffer::new(0);
    buf.init_executable([1u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        SystemProgram::from_account_view(&view),
        Err(ProgramError::IncorrectProgramId)
    ));
}
