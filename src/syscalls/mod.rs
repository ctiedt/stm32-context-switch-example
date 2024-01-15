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

/// Possible error during syscall.
#[derive(Debug)]
pub enum ReturnCode {
    /// Operation succeeded.
    Ok = 0,
    /// Call has not yet been implemented.
    NotImplemented,
    /// Number ten was passed to Increment.
    IncrementPastTen,
}

impl TryFrom<u32> for ReturnCode {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            x if x == Self::Ok as u32 => Ok(Self::Ok),
            x if x == Self::NotImplemented as u32 => Ok(Self::NotImplemented),
            x if x == Self::IncrementPastTen as u32 => Ok(Self::IncrementPastTen),
            _other => Err(())
        }
    }
}
