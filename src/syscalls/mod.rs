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
    /// Kernel is busy serving other calls. Try again.
    Busy,
}

#[derive(Debug)]
pub enum SyscallError {
    /// An unknown error code was encountered. Contains the invalid return code.
    Unknown(u32),
    /// Call has not yet been implemented.
    NotImplemented,
    /// Number ten was passed to Increment.
    IncrementPastTen,
    /// Kernel is busy.
    Busy,
}

/// Helper function to decode errors from an argument array.
fn decode_result(r0: u32, r1: u32) -> Result<u32, SyscallError> {
    match r0 {
        x if x == ReturnCode::Ok as u32 => Ok(r1),
        x if x == ReturnCode::NotImplemented as u32 => Err(SyscallError::NotImplemented),
        x if x == ReturnCode::IncrementPastTen as u32 => Err(SyscallError::IncrementPastTen),
        x if x == ReturnCode::Busy as u32 => Err(SyscallError::Busy),
        other => Err(SyscallError::Unknown(other)),
    }
}
