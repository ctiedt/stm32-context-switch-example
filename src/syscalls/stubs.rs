//! User-mode side of system calls.
//! Deals with receiving arguments and formatting them in a way suitable for kernel-side.
//! Also returning errors in a nice format.
//! Any validation here needs to be repeated in kernel for security.

use super::SyscallError;
use super::ReturnCode;
use super::kernel_mode::SyscallNumber;

macro_rules! build_args {
    ($n:pat $( , $arg:expr )*) => {
        [0u32 $( , $arg )*]
    };
}

macro_rules! exec_syscall {
    ($number:expr , $count:expr $( , $arg:expr )*) => {
        {
            let mut args : [u32; $count + 1] = [0u32 $( , $arg as u32 )*];
            let count = args.len() as u32;
            let pointer = args.as_mut_ptr() as u32;
            unsafe {
                core::arch::asm!(
                // Setup count and pointer to argument array.
                "mov r0, {count}",
                "mov r1, {pointer}",
                // Execute system call.
                "svc {number}",
                count = in(reg) count,
                pointer = in(reg) pointer,
                number = const $number as u32,
                // Clobber count and pointer registers.
                out("r0") _,
                out("r1") _,
                );
            }
            let code = args[0];
            let mut ret_args: [u32; $count] = [0u32;$count];
            ret_args.copy_from_slice(&args[1..]);
            if code == ReturnCode::Ok as u32 {
                Ok(ret_args)
            } else {
                Err(super::decode_error(code, &ret_args))
            }
        }
    };
}

/// Increment `value` by one and return it.
pub fn increment(value: u32) -> Result<u32, SyscallError> {
    exec_syscall!(SyscallNumber::Increment, 1, value).map(|args| args[0])
}

/// Read from USART2 into `buffer`.
/// Returns number of bytes read (at most `buffer.len()`) or error.
pub fn write(buffer: &[u8]) -> Result<usize, SyscallError> {
    exec_syscall!(SyscallNumber::Write, 2, buffer.len(), buffer.as_ptr())
        // read returns number of bytes in first argument.
        .map(|args| args[0] as usize)
}