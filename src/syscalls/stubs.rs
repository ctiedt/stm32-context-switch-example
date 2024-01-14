//! User-mode side of system calls.
//! Deals with receiving arguments and formatting them in a way suitable for kernel-side.
//! Also returning errors in a nice format.
//! Any validation here needs to be repeated in kernel for security.

use super::Error;
use super::kernel_mode::SyscallNumber;

/// Increment `value` by one and return it.
pub fn increment(value: u32) -> Result<u32, Error> {
    // Status code and value.
    let mut args = [0, value];
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
        number = const SyscallNumber::Increment as u32,
        out("r0") _,
        out("r1") _,
        );
    }

    // Return code in args[0] indicates success.
    if args[0] == 0 {
        Ok(args[1])
    } else {
        Err(args[0].try_into().expect("syscall failed with an invalid error code"))
    }
}
