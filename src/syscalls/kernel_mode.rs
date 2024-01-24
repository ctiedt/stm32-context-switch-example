//! Kernel-side code for system calls.
//! Deals with reading call number and arguments from stack and executing the actual calls.

use crate::{bios, scheduler};
use crate::scheduler::{CURRENT_TASK, KernelTask};
use crate::syscalls::SyscallError;
use super::ReturnCode;

#[naked]
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe fn SVCall() {
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
    handle_syscall = sym handle_syscall,
    options(noreturn)
    )
}

/// Decodes number and arguments for syscall from stack and calls corresponding handler.
pub unsafe extern fn handle_syscall(stack_pointer: *mut u32) {
    let number = get_syscall_number(stack_pointer).expect("invalid syscall number");
    let args = get_syscall_arguments(stack_pointer);

    match number {
        SyscallNumber::Write => handle_syscall_write(args),
        SyscallNumber::Increment => handle_syscall_increment(args),
        SyscallNumber::Block => handle_syscall_block(args),
    }
}

/// Extract syscall number from return address on stack.
unsafe fn get_syscall_number(stack_pointer: *const u32) -> Option<SyscallNumber> {
    /// Return address lies at 7th (index 6) position on the stack. Read it.
    /// It is a pointer to a 16 bit thumb instruction.
    let return_address = *stack_pointer.add(6) as *const u16;
    /// SVC instruction lies just before that. Compute its address.
    /// Cast it to *const u8 because we need to access its immediate byte.
    let svc_address = return_address.sub(1) as *const u8;
    /// Call number is first byte of this instruction.
    /// https://developer.arm.com/documentation/ddi0419/c/Application-Level-Architecture/The-Thumb-Instruction-Set-Encoding/16-bit-Thumb-instruction-encoding/Conditional-branch--and-Supervisor-Call?lang=en
    let number = *svc_address.add(0);
    SyscallNumber::from(number)
}

/// Extract syscall arguments from count and pointer on stack.
unsafe fn get_syscall_arguments(stack_pointer: *mut u32) -> &'static mut [u32] {
    core::slice::from_raw_parts_mut(stack_pointer, 2)
}


unsafe fn handle_syscall_increment(args: &mut [u32]) {
    let value = args[0];
    if value < 10 {
        args[0] = ReturnCode::Ok as u32;
        args[1] = value + 1;
    } else {
        args[0] = ReturnCode::IncrementPastTen as u32;
    }
}

unsafe fn handle_syscall_write(args: &mut [u32]) {
    let len = args[0] as usize;
    let ptr = args[1] as *const u8;
    let task = CURRENT_TASK;
    let kernel_task = KernelTask::WriteTx {
        args: args.as_mut_ptr(),
        total: len,
        left: len,
        data: ptr,
        task,
    };
    match scheduler::enqueue_task(kernel_task) {
        Ok(()) => {
            // Block calling thread and yield.
            (*task).set_blocked(true);
            cortex_m::peripheral::SCB::set_pendsv();
        }
        Err(_) => {
            args[0] = ReturnCode::Busy as u32;
        }
    }
}

unsafe fn handle_syscall_block(args: &mut [u32]) {
    let current = CURRENT_TASK;
    (*current).set_blocked(true);
    cortex_m::peripheral::SCB::set_pendsv();
}

/// Internal representation of system calls.
#[derive(Debug)]
pub(super) enum SyscallNumber {
    Increment,
    Write,
    Block,
}

impl SyscallNumber {
    pub fn from(imm: u8) -> Option<Self> {
        match imm {
            x if x == Self::Increment as u8 => Some(Self::Increment),
            x if x == Self::Write as u8 => Some(Self::Write),
            x if x == Self::Block as u8 => Some(Self::Block),
            other => None,
        }
    }
}