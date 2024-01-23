//! User-mode side of system calls.
//! Deals with receiving arguments and formatting them in a way suitable for kernel-side.
//! Also returning errors in a nice format.
//! Any validation here needs to be repeated in kernel for security.

use crate::syscalls::decode_result;
use super::SyscallError;
use super::ReturnCode;
use super::kernel_mode::SyscallNumber;

macro_rules! syscall {
    ($number:expr, $arg0:expr, $arg1:expr) => {
        unsafe {
            let mut code = 0u32;
            let mut value = 0u32;
            core::arch::asm!(
                // Move arguments to argument registers.
                "mov r0, {arg0}",
                "mov r1, {arg1}",
                // Execute system call.
                "svc {number}",
                // Move return code and optional value into variables.
                "mov {code}, r0",
                "mov {value}, r1",
                // Need to hard-code number for SVC.
                number = const $number as u32,
                // Tell Rust we require arg0 and arg1 in some registers.
                arg0 = in(reg) $arg0,
                arg1 = in(reg) $arg1,
                // Also result needs to be in registers.
                code = out(reg) code,
                value = out(reg) value,
                out("r0") _,
                out("r1") _,
            );
            decode_result(code, value)
        }
    };
}

/// Increment `value` by one and return it.
pub fn increment(value: u32) -> Result<u32, SyscallError> {
    syscall!(SyscallNumber::Increment, value, 0)
}

/// Read from USART2 into `buffer`.
/// Returns number of bytes read (at most `buffer.len()`) or error.
pub fn write(buffer: &[u8]) -> Result<usize, SyscallError> {
    let len = buffer.len();
    let data = buffer.as_ptr();
    syscall!(SyscallNumber::Write, len, data).map(|a| a as usize)
}

/// Block the current Task indefinitely for debugging purposes.
pub fn block() -> ! {
    match syscall!(SyscallNumber::Block, 0, 0) {
        _ => panic!("blocked task was resumed")
    }
}