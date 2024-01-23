use alloc::boxed::Box;
use core::ptr::null_mut;
use crate::scheduler::NEXT_TASK;

/// Holds all data necessary to start or continue a task.
#[repr(C)]
pub(crate) struct Task {
    stack_pointer: *mut u32,
    handler: Option<Box<dyn FnOnce()>>,
    next: Option<*mut Task>,
    is_blocked: bool,
}

impl Task {
    /// Create a new Task with a given handler to call upon switching to it.
    pub(super) fn new(stack: &mut [u32], handler: impl FnOnce() + 'static) -> Self {
        let top = Self::initialize_stack(stack);
        Self {
            stack_pointer: top.expect("misaligned stack"),
            handler: Some(Box::new(handler)),
            next: None,
            is_blocked: false,
        }
    }

    /// Create a new Task to represent the idle Task.
    /// It is assumed that this Task's stack pointer will be written to by the dispatcher before
    /// ever being switched to. Handler is left empty accordingly.
    pub(super) fn new_empty() -> Self {
        Self {
            stack_pointer: null_mut(),
            handler: None,
            next: None,
            is_blocked: false,
        }
    }

    /// Link to another Task from this one.
    /// Returns old link.
    pub(super) fn link_to(&mut self, next: Option<*mut Task>) -> Option<*mut Task> {
        core::mem::replace(&mut self.next, next)
    }

    /// Get pointer to next Task in linked list.
    pub(super) fn next(&self) -> Option<*mut Task> {
        self.next.clone()
    }

    /// Set block state of this Task. Return old state.
    pub(super) fn set_blocked(&mut self, value: bool) -> bool {
        core::mem::replace(&mut self.is_blocked, value)
    }

    /// Get blocked state of this Task.
    pub(super) fn is_blocked(&self) -> bool {
        self.is_blocked
    }

    /// Initialize a new stack to switch to and call the default task handler.
    /// Returns the new top of stack on success and `None` on misalignment.
    fn initialize_stack(stack: &mut [u32]) -> Option<*mut u32> {
        /// CPU uses a full descending stack, so SP points to last pushed element.
        /// This stack is empty, so `top` points just past end initially.
        let mut top = stack.as_mut_ptr_range().end;

        /// Stack pointer needs to be 4-byte aligned to work at all.
        if (top as u32 % 4) != 0 {
            return None;
        }

        /// Dispatcher requires a valid exception frame to return from, which needs an 8-byte aligned
        /// stack pointer.
        let alignment_bytes = (top as u32) % 8;
        let alignment_required = alignment_bytes != 0;
        if !alignment_required {
            top = top.wrapping_sub(1);
        }

        /// Helper to modify stack.
        macro_rules! push {
            ($value:expr) => {unsafe{top = top.wrapping_sub(1); *top = $value;}};
        }

        /// Could not find bit definition in `cortex_m`, so we need to use magic values to set our initial xPSR.
        /// **N,Z,C,V,Q** flags (bits 27-31) should be irrelevant, as long as user code does not rely
        /// on conditional execution in the first instruction.
        /// **GE** flags (bits 16-19) should also not matter, for the same reason.
        /// **ICI and TI** (interrupt-continuable and if-then-else flags, bits 25-26) could cause
        /// immediate push interruptable instructions (push, pop, etc.) or if-then-else blocks to
        /// misfire. Initializing them to 0 has worked so far.
        ///
        /// **Thumb state** (bit 24) must be 1 on thumb CPUs, as ARM instructions are not supported.
        /// **Bit 9** is used to indicate stack alignment after return from exception.
        /// Zero=4-byte aligned, One=8-byte aligned.
        /// **ISR_NUMBER** must be 0 in thread mode.
        let alignment_bit = if alignment_required { 0 } else { 1u32 << 9 };
        let xpsr = 1u32 << 24 | alignment_bit;

        /// Bottom of stack needs to be a valid exception frame.
        /// Initial xPSR value.
        push!(xpsr);
        /// Return address (take us to start_task_handler to start task).
        /// Requires LSB to be 1 to indicate thumb mode.
        push!(start_task_handler as u32 | 0x1);
        /// Link Register for [start_task_handler] to use for return.
        /// Set to an invalid address to cause a fault if it does, to aid debugging.
        push!(0xfffffffd);
        /// R12, R3-R0
        push!(12);
        push!(3);
        push!(2);
        push!(1);
        push!(0);

        /// The remaining registers also need to be saved, they should be in ascending order from
        /// SP forwards to allow a single instruction to push/pop them.
        /// Link Register with value for Thread mode on PSP without FPU.
        push!(0xFFFFFFFD);
        /// R11-R4
        push!(11);
        push!(10);
        push!(9);
        push!(8);
        push!(7);
        push!(6);
        push!(5);
        push!(4);

        Some(top)
    }
}

/// Retrieve handler of currently running task.
fn take_handler() -> Option<Box<dyn FnOnce()>> {
    unsafe {
        (*NEXT_TASK).handler.take()
    }
}

extern "C" fn start_task_handler() -> ! {
    let handler = take_handler().expect("task handler was empty");
    handler();
    loop {}
}