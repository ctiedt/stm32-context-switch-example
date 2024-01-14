//! System Calls are implemented here.
//!
//! ## How it works:
//! Users call the respective safe functions with fitting arguments.
//! Internally, parameters are converted to u32 and stored in an array of fitting size.
//! The length and pointer to this array are stored in R0 and R1 and an `svc <number>` instruction
//! is executed where `<number>` is a constant unique to this system call.
//! R2 contains a pointer to a result u32 indicating success.
//!
//! The `svc` instruction triggers the [SVCall] exception handler, which looks at the stack to determine
//! length and pointer to the arguments and calls [handle_syscall] with the call number.
//! This function decodes the number into the appropriate action and passes along the parameters.
//! The challenge here lies in determining proper calling convention for [SVCall] to [handle_syscall]
//! for arguments and return values.

use cortex_m_rt::exception;


/// Internal representation of system calls.
#[derive(Debug)]
enum SyscallNumber {
    Increment,
}

/// Possible error during syscall.
#[derive(Debug)]
pub enum Error {
    Unknown = 1,
}

impl TryFrom<u32> for Error {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            x if x == Self::Unknown as u32 => Ok(Self::Unknown),
            other => Err(())
        }
    }
}

#[derive(Debug)]
#[repr(u32)]
enum InternalError {
    BadNumber(u8),
}

impl TryFrom<u8> for SyscallNumber {
    type Error = InternalError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Self::Increment as u8 => Ok(Self::Increment),
            other => Err(InternalError::BadNumber(other))
        }
    }
}


pub mod stub {
    use super::*;

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
}

#[naked]
#[no_mangle]
#[allow(non_snake_case)]
unsafe fn SVCall() {
    /// Link Register decoding
    /// F1 = 1 0001 = Handler, No FP, MSP
    /// F9 = 1 1001 = Thread, No FP, MSP
    /// FD = 1 1101 = Thread, No FP, PSP
    /// E1 = 0 0001 = Handler, FP, MSP
    /// E9 = 0 1001 = Thread, FP, MSP
    /// ED = 0 1101 = Thread, FP, PSP
    ///
    /// 1 0001
    /// ^ ^^
    /// | |+------- Stack
    /// | +-------- Mode
    /// +---------- FP
    core::arch::asm!(
    // Determine which stack pointer to use (PSP or MSP) by looking at bit 2 in LR.
    // EXC_RETURN value looks like 31:5=1, 4=0 if FP, 3=0 if Handler, 2=0 if MSP.
    // Following code taken from: https://developer.arm.com/documentation/ka004005/latest/
    // We cannot simple `msr r0, SP`, because it is banked using the SPSEL bit in CONTROL and we are
    // always running in handler mode on MSP here.
    // Test bit 2 of LR and start conditional execution.
    "tst lr, #4",
    // If-then-else on I assume Z flag?
    "ite eq",
    // Load corresponding stack pointer into R0.
    "mrseq r0, MSP",
    "mrsne r0, PSP",
    // R0 now contains relevant stack pointer.

    // Following code was reverse-engineered using godbolt.org.
    // TODO: Read up on calling-convention of thumbv7em-none-eabi
    "push    {{r7, lr}}",
    "mov     r7, sp",
    "sub     sp, #8",
    // Call handle_syscall which does argument decoding etc.
    "bl      {handle_syscall}",
    "add     sp, #8",
    "pop     {{r7, pc}}",
    // // Read call number by looking at the return address on the stack.
    // // Load return address (at SP+6 words) into R0.
    // "ldr r0, [r3, #(6*4)]",
    // // Load number of svc instruction into r0.
    // // Offset -2 was found experimentally.
    // "ldrb r0, [r0, #-2]",
    handle_syscall = sym handle_syscall,
    options(noreturn)
    )
}

/// Decodes number and arguments for syscall from stack and calls corresponding handler.
unsafe extern fn handle_syscall(stack_pointer: *mut u32) {
    let number = get_syscall_number(stack_pointer).expect("invalid syscall number");
    let args = get_syscall_arguments(stack_pointer);
    let (result, args) = args.split_at_mut(1);

    // Execute corresponding syscall handler.
    // Data return values from handlers are returned using args.
    let call_result = match number {
        SyscallNumber::Increment => {
            handle_syscall_increment(args)
        }
    };

    match call_result {
        Ok(_) => result[0] = 0,
        Err(e) => result[0] = e as u32,
    }
}

/// Extract syscall number from return address on stack.
unsafe fn get_syscall_number(stack_pointer: *const u32) -> Result<SyscallNumber, InternalError> {
    /// Return address lies at 7th (index 6) position on the stack. Read it.
    /// It is a pointer to a 16 bit thumb instruction.
    let return_address = *stack_pointer.add(6) as *const u16;
    /// SVC instruction lies just before that. Compute its address.
    /// Cast it to *const u8 because we need to access its immediate byte.
    let svc_address = return_address.sub(1) as *const u8;
    /// Call number is first byte of this instruction.
    /// https://developer.arm.com/documentation/ddi0419/c/Application-Level-Architecture/The-Thumb-Instruction-Set-Encoding/16-bit-Thumb-instruction-encoding/Conditional-branch--and-Supervisor-Call?lang=en
    let number = *svc_address.add(0);
    number.try_into()
}

/// Extract syscall arguments from count and pointer on stack.
unsafe fn get_syscall_arguments(stack_pointer: *const u32) -> &'static mut [u32] {
    let count = *stack_pointer as usize;
    let pointer = *stack_pointer.add(1) as *mut u32;
    core::slice::from_raw_parts_mut(pointer, count)
}

unsafe fn handle_syscall_increment(args: &mut [u32]) -> Result<(), Error> {
    args[0] += 1;
    Ok(())
}