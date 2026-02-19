#[inline(always)]
pub fn set_return_data(_data: &[u8]) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        use solana_define_syscall::definitions::sol_set_return_data;
        unsafe {
            sol_set_return_data(_data.as_ptr(), _data.len() as u64);
        }
    }
}
