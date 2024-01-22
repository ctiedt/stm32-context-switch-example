//! System Calls are implemented here.
//!
//! ## How it works:
//! Users call the respective safe functions with fitting arguments.
//! Internally, parameters are converted to u32 and stored in an array of fitting size.
//! The length and pointer to this array are stored in R0 and R1 and an `svc <number>` instruction
//! is executed where `<number>` is a constant unique to this system call.
//! R2 contains a pointer to a result u32 indicating success.
//!
//! The `svc` instruction triggers the [kernel_mode::SVCall] exception handler, which looks at the stack to determine
//! length and pointer to the arguments and calls [kernel_mode::handle_syscall] with the call number.
//! This function decodes the number into the appropriate action and passes along the parameters.
//! The challenge here lies in determining proper calling convention for [kernel_mode::SVCall] to [kernel_mode::handle_syscall]
//! for arguments and return values.

mod kernel_mode;
pub mod stubs;

/// Returned from system call.
/// Users should not use this directly but instead handle [Result<_, SyscallError>] where possible.
#[derive(Debug)]
#[repr(u32)]
pub enum ReturnCode {
    /// Operation succeeded.
    Ok = 0,
    /// Call has not yet been implemented.
    NotImplemented,
    /// Number ten was passed to Increment.
    IncrementPastTen,
    /// Insufficient space while writing.
    InsufficientSpace,
}

#[derive(Debug)]
pub enum SyscallError {
    /// An unknown error code was encountered. Contains the invalid return code.
    Unknown(u32),
    /// Call has not yet been implemented.
    NotImplemented,
    /// Number ten was passed to Increment.
    IncrementPastTen,
    /// Insufficient space while writing. Contains number of elements successfully written.
    InsufficientSpace(usize),
}

/// Helper function to decode errors from an argument array.
fn decode_error(code: u32, args: &[u32]) -> SyscallError {
    match code {
        x if x == ReturnCode::NotImplemented as u32 => SyscallError::NotImplemented,
        x if x == ReturnCode::IncrementPastTen as u32 => SyscallError::IncrementPastTen,
        x if x == ReturnCode::InsufficientSpace as u32 => SyscallError::InsufficientSpace(args[0] as usize),
        other => SyscallError::Unknown(other),
    }
}
