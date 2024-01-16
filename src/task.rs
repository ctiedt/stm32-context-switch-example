use core::ptr::{null_mut};
use core::mem::MaybeUninit;
use core::sync::atomic::Ordering;
use crate::{bios, global_peripherals};
use cortex_m::register::control::{Fpca, Npriv, Spsel};
use core::fmt::Write;

pub(crate) const MAX_TASKS: usize = 8;

pub(crate) static mut TASK_TABLE: TaskTable = TaskTable::new();

pub(crate) struct TaskTable {
    tasks: [MaybeUninit<Task>; MAX_TASKS],
    current: usize,
    size: usize,
}

impl TaskTable {
    /// Create a new TaskTable without any tasks.
    const fn new() -> Self {
        let tasks = MaybeUninit::uninit_array();
        Self {
            tasks,
            current: 0,
            size: 0,
        }
    }

    pub fn insert_task(&mut self, task: Task) {
        unsafe { *self.tasks[self.size].assume_init_mut() = task };
        self.size += 1;
    }

    pub fn current_task(&mut self) -> &mut Task {
        unsafe { self.tasks[self.current].assume_init_mut() }
    }

    pub fn next_task(&mut self) -> Option<&mut Task> {
        if self.size == 0 {
            return None;
        }
        self.current = (self.current + 1) % self.size;
        unsafe { Some(self.tasks[self.current].assume_init_mut()) }
    }
}

#[repr(C)]
pub(crate) struct Task {
    stack_pointer: *mut u32,
}

impl Task {
    /// Create a new dummy Task.
    /// Stack pointer is invalid and is assumed to be overwritten before first switch to this Task.
    fn new_dummy() -> Self {
        Self {
            stack_pointer: null_mut()
        }
    }

    /// Create a new Task with a given stack pointer.
    fn new(stack: *mut u32) -> Self {
        Self {
            stack_pointer: stack
        }
    }
}

pub(crate) static mut OS_CURRENT_TASK: *mut Task = core::ptr::null_mut();
pub(crate) static mut OS_NEXT_TASK: *mut Task = core::ptr::null_mut();

/// Hand off control to the scheduler.
/// Sets up process stack to use provided stack, switches to unprivileged thread mode and starts
/// execution of `entry`.
pub(crate) fn start_scheduler(app_stack: &mut [u32], entry: impl FnOnce() -> !) -> ! {
    initialize_scheduler();

    /// Setup process stack before switching to it.
    /// Hopefully, we can avoid disabling interrupts for this.
    let mut top = app_stack.as_mut_ptr_range().end as u32;
    top = top - top % 8;
    unsafe { cortex_m::register::psp::write(top) }

    /// Switch to unprivileged thread mode without floating point.
    let mut control = cortex_m::register::control::read();
    if control.fpca() != Fpca::NotActive {
        todo!("floating point mode is not supported yet")
    }
    control.set_spsel(Spsel::Psp);
    control.set_npriv(Npriv::Unprivileged);
    /// Note: [cortex_m::register::control::write] accesses stack around asm, which will not work
    /// during stack switching.
    unsafe {
        core::arch::asm!(
        "msr CONTROL, {}",
        "isb",
        in(reg) control.bits(),
        options(nomem, nostack, preserves_flags)
        )
    }
    /// Ensure memory accesses are not reordered around the CONTROL update.
    /// Copied from [cortex_m::register::control::write].
    core::sync::atomic::compiler_fence(Ordering::SeqCst);

    /// We are now in unprivileged thread mode!
    /// Call our main thread.
    entry()
}

/// Initialize scheduler structures with dummy data to allow a context switch.
fn initialize_scheduler() {
    /// Setup task table using a dummy task. At least one task is required for a context switch to
    /// work, since the stack pointer is written/read to/from the last/next task.
    let app_task = Task::new_dummy();
    unsafe {
        TASK_TABLE.insert_task(app_task);
        OS_NEXT_TASK = TASK_TABLE.next_task().expect("failed to initialize dummy task");
        // Required to have a valid reference during first scheduler run.
        OS_CURRENT_TASK = OS_NEXT_TASK;
    }
}


pub(crate) fn schedule_next_task() {
    unsafe { OS_CURRENT_TASK = OS_NEXT_TASK; }
    unsafe { OS_NEXT_TASK = TASK_TABLE.next_task().expect("failed to get next task") };

    let mut serial2 = bios::raw_output();
    writeln!(serial2, "Now running task {:?}\r", unsafe { OS_NEXT_TASK }).unwrap();
}

pub(crate) fn create_task(handler: fn() -> (), _params: *const (), stack: &mut [u32]) {
    // Stacks grow down, so we take the pointer just past the end
    let mut top = stack.as_mut_ptr_range().end;

    macro_rules! push {
        ($value:expr) => {unsafe{top = top.wrapping_sub(1); *top = $value;}};
    }

    // xPSR
    push!(1u32 << 24);
    // PC
    push!((handler as u32) | 0x1);
    // Address for task finished function
    push!((task_finished as u32) | 0x1);
    // R12, R3-R0
    push!(12);
    push!(3);
    push!(2);
    push!(1);
    push!(0);
    // R4-R11 + LR (popped in reverse)
    // push!(0xFFFFFFF9u32);
    push!(11);
    push!(10);
    push!(9);
    push!(8);
    push!(7);
    push!(6);
    push!(5);
    push!(4);

    let task = Task::new(top);
    unsafe { TASK_TABLE.insert_task(task); }
}

fn task_finished() {
    loop {
        cortex_m::asm::bkpt();
    }
}