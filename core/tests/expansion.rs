use quasar_core::prelude::*;
use quasar_core::traits::{Discriminator, Event, Space};
use solana_program_error::ProgramError;

// We need a program ID for #[account] to reference.
solana_address::declare_id!("11111111111111111111111111111112");

// ---------------------------------------------------------------------------
// #[account] — discriminator correctness
// ---------------------------------------------------------------------------

#[account(discriminator = [1])]
pub struct SingleByteDisc {
    pub value: u64,
}

#[test]
fn account_single_byte_discriminator() {
    assert_eq!(SingleByteDisc::DISCRIMINATOR, &[1]);
}

#[account(discriminator = [1, 2, 3, 4])]
pub struct MultiByteDisc {
    pub value: u64,
}

#[test]
fn account_multi_byte_discriminator() {
    assert_eq!(MultiByteDisc::DISCRIMINATOR, &[1, 2, 3, 4]);
}

#[account(discriminator = [255])]
pub struct MaxValueDisc {}

#[test]
fn account_max_value_byte_discriminator() {
    assert_eq!(MaxValueDisc::DISCRIMINATOR, &[255]);
}

#[account(discriminator = [1, 2])]
pub struct DiscByteOrder {}

#[test]
fn account_discriminator_byte_ordering() {
    let disc = DiscByteOrder::DISCRIMINATOR;
    assert_eq!(disc[0], 1);
    assert_eq!(disc[1], 2);
}

// ---------------------------------------------------------------------------
// #[account] — space calculation
// ---------------------------------------------------------------------------

#[account(discriminator = [1])]
pub struct EmptyAccount {}

#[test]
fn account_space_empty() {
    assert_eq!(EmptyAccount::SPACE, 1); // disc only
}

#[account(discriminator = [1])]
pub struct SingleAddress {
    pub key: Address,
}

#[test]
fn account_space_single_address() {
    assert_eq!(SingleAddress::SPACE, 1 + 32);
}

#[account(discriminator = [1])]
pub struct SingleU64 {
    pub amount: u64,
}

#[test]
fn account_space_single_u64() {
    assert_eq!(SingleU64::SPACE, 1 + 8);
}

#[account(discriminator = [1])]
pub struct MixedFields {
    pub key: Address,
    pub amount: u64,
    pub flag: u8,
    pub active: bool,
}

#[test]
fn account_space_mixed_fields() {
    // disc(1) + Address(32) + u64(8) + u8(1) + bool(1) = 43
    assert_eq!(MixedFields::SPACE, 1 + 32 + 8 + 1 + 1);
}

#[account(discriminator = [1, 2])]
pub struct TwoByteDiscWithFields {
    pub amount: u64,
}

#[test]
fn account_space_two_byte_disc_with_fields() {
    assert_eq!(TwoByteDiscWithFields::SPACE, 2 + 8);
}

// ---------------------------------------------------------------------------
// #[event] — DATA_SIZE
// ---------------------------------------------------------------------------

#[event(discriminator = [10])]
pub struct EventU64 {
    pub amount: u64,
}

#[test]
fn event_data_size_u64() {
    assert_eq!(<EventU64 as Event>::DATA_SIZE, 8);
}

#[event(discriminator = [11])]
pub struct EventAddressU64 {
    pub who: Address,
    pub amount: u64,
}

#[test]
fn event_data_size_address_u64() {
    assert_eq!(<EventAddressU64 as Event>::DATA_SIZE, 40);
}

#[event(discriminator = [12])]
pub struct EventBool {
    pub flag: bool,
}

#[test]
fn event_data_size_bool() {
    assert_eq!(<EventBool as Event>::DATA_SIZE, 1);
}

#[event(discriminator = [13])]
pub struct EventMultiU64 {
    pub a: u64,
    pub b: u64,
}

#[test]
fn event_data_size_multi() {
    assert_eq!(<EventMultiU64 as Event>::DATA_SIZE, 8 + 8);
}

// ---------------------------------------------------------------------------
// #[event] — write_data correctness
// ---------------------------------------------------------------------------

#[test]
fn event_write_data_u64() {
    let evt = EventU64 { amount: 42 };
    let mut buf = [0u8; 8];
    evt.write_data(&mut buf);
    assert_eq!(&buf, &42u64.to_le_bytes());
}

#[test]
fn event_write_data_address_u64() {
    let addr = Address::new_from_array([0xAA; 32]);
    let evt = EventAddressU64 {
        who: addr,
        amount: 100,
    };
    let mut buf = [0u8; 40];
    evt.write_data(&mut buf);
    assert_eq!(&buf[..32], &[0xAA; 32]);
    assert_eq!(&buf[32..], &100u64.to_le_bytes());
}

#[test]
fn event_write_data_bool() {
    let evt = EventBool { flag: true };
    let mut buf = [0u8; 1];
    evt.write_data(&mut buf);
    assert_eq!(buf[0], 1);

    let evt_false = EventBool { flag: false };
    evt_false.write_data(&mut buf);
    assert_eq!(buf[0], 0);
}

// ---------------------------------------------------------------------------
// #[event] — discriminator
// ---------------------------------------------------------------------------

#[test]
fn event_discriminator() {
    assert_eq!(<EventU64 as Event>::DISCRIMINATOR, &[10]);
    assert_eq!(<EventAddressU64 as Event>::DISCRIMINATOR, &[11]);
}

// ---------------------------------------------------------------------------
// #[error_code] — numbering
// ---------------------------------------------------------------------------

#[error_code]
pub enum TestError {
    First,
    Second,
    Third,
}

#[test]
fn error_code_default_numbering() {
    assert_eq!(TestError::First as u32, 0);
    assert_eq!(TestError::Second as u32, 1);
    assert_eq!(TestError::Third as u32, 2);
}

#[error_code]
pub enum TestErrorExplicit {
    First = 100,
    Second,
    Third,
}

#[test]
fn error_code_explicit_start() {
    assert_eq!(TestErrorExplicit::First as u32, 100);
    assert_eq!(TestErrorExplicit::Second as u32, 101);
    assert_eq!(TestErrorExplicit::Third as u32, 102);
}

#[error_code]
pub enum TestErrorGap {
    First = 0,
    Second = 10,
    Third,
}

#[test]
fn error_code_gap() {
    assert_eq!(TestErrorGap::First as u32, 0);
    assert_eq!(TestErrorGap::Second as u32, 10);
    assert_eq!(TestErrorGap::Third as u32, 11);
}

#[error_code]
pub enum TestErrorSingle {
    Only = 42,
}

#[test]
fn error_code_single_variant() {
    assert_eq!(TestErrorSingle::Only as u32, 42);
}

// ---------------------------------------------------------------------------
// #[error_code] — From<MyError> for ProgramError
// ---------------------------------------------------------------------------

#[test]
fn error_code_into_program_error() {
    let pe: ProgramError = TestError::First.into();
    assert_eq!(pe, ProgramError::Custom(0));

    let pe: ProgramError = TestError::Third.into();
    assert_eq!(pe, ProgramError::Custom(2));

    let pe: ProgramError = TestErrorExplicit::First.into();
    assert_eq!(pe, ProgramError::Custom(100));
}

// ---------------------------------------------------------------------------
// #[error_code] — TryFrom<u32>
// ---------------------------------------------------------------------------

#[test]
fn error_code_try_from_valid() {
    let err = TestError::try_from(0u32).unwrap();
    assert_eq!(err as u32, 0);

    let err = TestError::try_from(2u32).unwrap();
    assert_eq!(err as u32, 2);
}

#[test]
fn error_code_try_from_invalid() {
    let result = TestError::try_from(3u32);
    assert!(matches!(result, Err(ProgramError::InvalidArgument)));
}

#[test]
fn error_code_try_from_past_last_variant() {
    // TestErrorGap has Third = 11, so 12 should fail
    let result = TestErrorGap::try_from(12u32);
    assert!(matches!(result, Err(ProgramError::InvalidArgument)));

    // But 10 and 11 should succeed
    assert!(TestErrorGap::try_from(10u32).is_ok());
    assert!(TestErrorGap::try_from(11u32).is_ok());
}

// ---------------------------------------------------------------------------
// #[account] — discriminator validation (Account<T>::from_account_view)
// ---------------------------------------------------------------------------

use quasar_core::__internal::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED};
use quasar_core::accounts::Account;

struct AccountBuffer {
    inner: std::vec::Vec<u64>,
}

impl AccountBuffer {
    fn new(data_len: usize) -> Self {
        let byte_len =
            std::mem::size_of::<RuntimeAccount>() + data_len + MAX_PERMITTED_DATA_INCREASE + std::mem::size_of::<u64>();
        let u64_count = (byte_len + 7) / 8;
        Self {
            inner: std::vec![0; u64_count],
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

    unsafe fn view(&mut self) -> AccountView {
        AccountView::new_unchecked(self.raw())
    }

    fn write_data(&mut self, data: &[u8]) {
        let data_start = std::mem::size_of::<RuntimeAccount>();
        let dst = unsafe {
            let ptr = (self.inner.as_mut_ptr() as *mut u8).add(data_start);
            std::slice::from_raw_parts_mut(ptr, data.len())
        };
        dst.copy_from_slice(data);
    }
}

// SingleByteDisc defined above: discriminator = [1], one u64 field.
// Its SPACE = 1 (disc) + 8 (u64) = 9.

#[test]
fn account_rejects_wrong_discriminator() {
    let owner_bytes = <SingleByteDisc as quasar_core::traits::Owner>::OWNER.to_bytes();
    let data_len = SingleByteDisc::SPACE;
    let mut buf = AccountBuffer::new(data_len);
    buf.init([0u8; 32], owner_bytes, 1000, data_len as u64, false, false);

    // Write wrong discriminator (2 instead of 1)
    let mut data = vec![0u8; data_len];
    data[0] = 2;
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(matches!(
        Account::<SingleByteDisc>::from_account_view(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

#[test]
fn account_rejects_zero_discriminator() {
    let owner_bytes = <SingleByteDisc as quasar_core::traits::Owner>::OWNER.to_bytes();
    let data_len = SingleByteDisc::SPACE;
    let mut buf = AccountBuffer::new(data_len);
    buf.init([0u8; 32], owner_bytes, 1000, data_len as u64, false, false);

    // All-zero data — uninitialized account should be rejected
    let data = vec![0u8; data_len];
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(matches!(
        Account::<SingleByteDisc>::from_account_view(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

#[test]
fn account_accepts_correct_discriminator() {
    let owner_bytes = <SingleByteDisc as quasar_core::traits::Owner>::OWNER.to_bytes();
    let data_len = SingleByteDisc::SPACE;
    let mut buf = AccountBuffer::new(data_len);
    buf.init([0u8; 32], owner_bytes, 1000, data_len as u64, false, false);

    // Write correct discriminator
    let mut data = vec![0u8; data_len];
    data[0] = 1;
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(Account::<SingleByteDisc>::from_account_view(&view).is_ok());
}

// MultiByteDisc: discriminator = [1, 2, 3, 4], one u64 field.
// SPACE = 4 + 8 = 12.

#[test]
fn account_rejects_partial_discriminator_match() {
    let owner_bytes = <MultiByteDisc as quasar_core::traits::Owner>::OWNER.to_bytes();
    let data_len = MultiByteDisc::SPACE;
    let mut buf = AccountBuffer::new(data_len);
    buf.init([0u8; 32], owner_bytes, 1000, data_len as u64, false, false);

    // First 3 bytes correct, last byte wrong
    let mut data = vec![0u8; data_len];
    data[0] = 1;
    data[1] = 2;
    data[2] = 3;
    data[3] = 99; // should be 4
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(matches!(
        Account::<MultiByteDisc>::from_account_view(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

#[test]
fn account_rejects_too_small_data() {
    let owner_bytes = <SingleByteDisc as quasar_core::traits::Owner>::OWNER.to_bytes();
    // data_len smaller than SPACE
    let short_len = SingleByteDisc::SPACE - 1;
    let mut buf = AccountBuffer::new(short_len);
    buf.init([0u8; 32], owner_bytes, 1000, short_len as u64, false, false);

    let mut data = vec![0u8; short_len];
    data[0] = 1;
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(matches!(
        Account::<SingleByteDisc>::from_account_view(&view),
        Err(ProgramError::AccountDataTooSmall)
    ));
}

#[test]
fn account_rejects_wrong_owner() {
    let data_len = SingleByteDisc::SPACE;
    let mut buf = AccountBuffer::new(data_len);
    // Wrong owner — not the program ID
    buf.init([0u8; 32], [0xFFu8; 32], 1000, data_len as u64, false, false);

    let mut data = vec![0u8; data_len];
    data[0] = 1;
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(matches!(
        Account::<SingleByteDisc>::from_account_view(&view),
        Err(ProgramError::IllegalOwner)
    ));
}

#[test]
fn account_from_view_mut_rejects_not_writable() {
    let owner_bytes = <SingleByteDisc as quasar_core::traits::Owner>::OWNER.to_bytes();
    let data_len = SingleByteDisc::SPACE;
    let mut buf = AccountBuffer::new(data_len);
    buf.init([0u8; 32], owner_bytes, 1000, data_len as u64, false, false); // not writable

    let mut data = vec![0u8; data_len];
    data[0] = 1;
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(matches!(
        Account::<SingleByteDisc>::from_account_view_mut(&view),
        Err(ProgramError::Immutable)
    ));
}
