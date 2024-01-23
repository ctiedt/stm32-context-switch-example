//! Functions related to creation and switching between threads.

use alloc::boxed::Box;
use core::fmt::Write;
use cortex_m::register::control::{Fpca, Npriv, Spsel};
use core::sync::atomic::Ordering;
use cortex_m::peripheral::SYST;
use stm32f4xx_hal::prelude::_fugit_RateExtU32;
use stm32f4xx_hal::rcc::Clocks;
use stm32f4xx_hal::timer::{SysEvent, SysTimerExt};
use crate::task::{Task};
use heapless::spsc::Queue;

/// How many tasks may be queued for execution in kernel mode.
const KERNEL_TASK_COUNT: usize = 16;
/// Queue of tasks to run in kernel mode.
static mut KERNEL_TASK_QUEUE: Queue<KernelTask, KERNEL_TASK_COUNT> = Queue::new();

/// Pointers to previous (switched from) and current (switched to) Tasks.
pub(super) static mut PREVIOUS_TASK: *mut Task = core::ptr::null_mut();
pub(super) static mut CURRENT_TASK: *mut Task = core::ptr::null_mut();


/// Idle [Task] used when no others can run.
static mut IDLE_TASK: Option<Task> = None;


/// Get idle Task pointer. Requires [IDLE_TASK] to be [Some].
unsafe fn get_idle_task_ptr() -> *mut Task {
    IDLE_TASK.as_mut().expect("idle task was not initialized")
}

/// Ring-shaped linked list of all available tasks.
static mut TASK_LIST: Option<*mut Task> = None;

pub fn start(clocks: &Clocks, syst: SYST, app_stack: &mut [u32], app: impl FnOnce() + 'static) -> ! {
    /// Create and save idle Task to switch to later.
    unsafe {
        let idle_task = Task::new_empty();
        IDLE_TASK = Some(idle_task);
    }

    /// Create application Task and link it to itself.
    let app_task = Task::new(app_stack, app);
    let app_task_ptr = Box::into_raw(Box::new(app_task));
    unsafe {
        (*app_task_ptr).link_to(Some(app_task_ptr));
    }

    /// Set idle task as initial "switched from" and "switched to" task because this function will
    /// become the idle task.
    unsafe {
        PREVIOUS_TASK = get_idle_task_ptr();
        CURRENT_TASK = get_idle_task_ptr();
    }
    

    /// Notify kernel to turn off privileged execution for threads.
    enqueue_task(KernelTask::LowerThreadPrivileges).expect("failed to enqueue privilege lowering task");

    /// Start SysTick and with that preemptive scheduling.
    let mut systick = syst.counter_hz(&clocks);
    systick.listen(SysEvent::Update);
    systick.start(1.Hz()).unwrap();

    /// Enter idle loop.
    loop {}
}

/// Any Task the kernel needs to perform during PendSV.
#[derive(Debug)]
pub enum KernelTask {
    /// Switch threads to unprivileged mode.
    LowerThreadPrivileges,
}

/// Attempt to enqueue a new kernel mode task.
/// Returns [Err] containing `task` if queue is full.
pub fn enqueue_task(task: KernelTask) -> Result<(), KernelTask> {
    unsafe { KERNEL_TASK_QUEUE.enqueue(task) }
}

/// Execute a kernel mode task, if available.
pub fn execute_task() {
    let task = match unsafe { KERNEL_TASK_QUEUE.dequeue() } {
        None => { return; }
        Some(task) => task
    };

    match task {
        KernelTask::LowerThreadPrivileges => {
            let mut control = cortex_m::register::control::read();
            control.set_npriv(Npriv::Unprivileged);
            unsafe { cortex_m::register::control::write(control); }
        }
    }
}

pub(crate) fn schedule_next() {}
