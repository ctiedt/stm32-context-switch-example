//! User-mode side of system calls.
//! Deals with receiving arguments and formatting them in a way suitable for kernel-side.
//! Also returning errors in a nice format.
//! Any validation here needs to be repeated in kernel for security.

use crate::syscalls::decode_result;
use super::SyscallError;
use super::ReturnCode;
use super::kernel_mode::SyscallNumber;

macro_rules! build_args {
    ($n:pat $( , $arg:expr )*) => {
        [0u32 $( , $arg )*]
    };
}
//
// macro_rules! exec_syscall {
//     ($number:expr, $arg1:expr, $arg2:expr) => {
//         {
//             let mut arg1 = $arg1;
//             let mut arg2 = $arg2;
//             unsafe {
//                 core::arch::asm!(
//                 // Save number to r0, arg{1,2,3} to r{1,2,3}.
//                 "mov r0, {number}",
//                 "mov r1, {arg1}",
//                 "mov r2, {arg2}",
//                 "mov r3, {arg3}",
//                 // Execute system call.
//                 "svc {number}",
//                 number = const $number as u32,
//                 arg1 = inout(reg) $arg1,
//                 arg2 = inout(reg) $arg2,
//                 arg3 = inout(reg) $arg3,
//                 // Clobber argument register so the compiler can save them.
//                 out("r0") _,
//                 out("r1") _,
//                 out("r2") _,
//                 out("r3") _,
//                 );
//             }
//             let code = args[0];
//             let mut ret_args: [u32; $count] = [0u32;$count];
//             ret_args.copy_from_slice(&args[1..]);
//             if code == ReturnCode::Ok as u32 {
//                 Ok(ret_args)
//             } else {
//                 Err(super::decode_error(code, &ret_args))
//             }
//         }
//     };
// }

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