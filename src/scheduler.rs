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
use crate::bios;
use crate::syscalls::ReturnCode;

/// How many tasks may be queued for execution in kernel mode.
const KERNEL_TASK_COUNT: usize = 16;
/// Queue of tasks to run in kernel mode.
static mut KERNEL_TASK_QUEUE: Queue<KernelTask, KERNEL_TASK_COUNT> = Queue::new();

/// Currently running Task.
pub(super) static mut CURRENT_TASK: *mut Task = core::ptr::null_mut();


/// Idle [Task] used when no others can run.
static mut IDLE_TASK: Option<Task> = None;


/// Get idle Task pointer. Requires [IDLE_TASK] to be [Some].
unsafe fn get_idle_task_ptr() -> *mut Task {
    IDLE_TASK.as_mut().expect("idle task was not initialized")
}

/// Ring-shaped linked list of all available tasks.
static mut TASK_LIST: *mut Task = core::ptr::null_mut();

/// Insert a new Task into the list of available Tasks.
fn insert_task(mut task: Task) {
    unsafe { task.link_to(TASK_LIST) };
    let boxed = Box::new(task);
    let ptr = Box::into_raw(boxed);
    unsafe { TASK_LIST = ptr; }
}


pub fn start(clocks: &Clocks, syst: SYST, app_stack: &mut [u32], app: impl FnOnce() + 'static) -> ! {
    /// Create and save idle Task to switch to later.
    unsafe {
        let idle_task = Task::new_empty();
        IDLE_TASK = Some(idle_task);
    }

    /// Create application Task and insert it into Task list.
    let app_task = Task::new(app_stack, app);
    insert_task(app_task);

    /// Set idle task as initially running Task.
    unsafe {
        CURRENT_TASK = get_idle_task_ptr();
    }

    /// Notify kernel to turn off privileged execution for threads.
    enqueue_kernel_task(KernelTask::LowerThreadPrivileges).expect("failed to enqueue privilege lowering task");

    /// Start SysTick and with that preemptive scheduling.
    let mut systick = syst.counter_hz(&clocks);
    systick.listen(SysEvent::Update);
    systick.start(1000.Hz()).unwrap();

    /// Enter idle loop.
    loop {}
}

/// Any Task the kernel needs to perform during PendSV.
#[derive(Debug)]
pub enum KernelTask {
    /// Switch threads to unprivileged mode.
    LowerThreadPrivileges,
    /// Write data from a buffer to the TX buffer.
    WriteTx {
        /// Arguments array (on stack) of corresponding syscall.
        args: *mut u32,
        /// Total number of bytes to send.
        total: usize,
        /// Number of bytes left to send.
        left: usize,
        /// Start of data.
        data: *const u8,
        /// Task to unblock.
        task: *mut Task,
    },
    /// Start a new Task.
    Spawn {
        /// Handler to call.
        f: fn(),
        /// Start of stack memory.
        stack: *mut u32,
        /// Size of provided stack.
        stack_size: usize,
    },
    /// Unblock a Task from running.
    Unblock(*mut Task),
}

/// Attempt to enqueue a new kernel mode task.
/// Returns [Err] containing `task` if queue is full.
pub fn enqueue_kernel_task(task: KernelTask) -> Result<(), KernelTask> {
    unsafe { KERNEL_TASK_QUEUE.enqueue(task) }
}

/// Execute a kernel mode task, if available.
/// Returns new Task if the current one should be continued in the next dispatcher call.
pub fn execute_task(task: KernelTask) -> Option<KernelTask> {
    match task {
        KernelTask::LowerThreadPrivileges => {
            let mut control = cortex_m::register::control::read();
            control.set_npriv(Npriv::Unprivileged);
            unsafe { cortex_m::register::control::write(control); }
            None
        }
        KernelTask::WriteTx { args, total, left: len, data, task } => {
            // Block Task to let others run in the meantime.
            unsafe { (*task).set_blocked(true); }
            let buffer = unsafe { core::slice::from_raw_parts(data, len) };
            let mut tx = bios::buffered_output();
            match tx.append(buffer) {
                Ok(_) => unsafe {
                    // Notify Task of success.
                    let args = core::slice::from_raw_parts_mut(args, 2);
                    args[0] = ReturnCode::Ok as u32;
                    args[1] = total as u32;
                    // Unblock Task in the next switch.
                    Some(KernelTask::Unblock(task))
                }
                Err(appended) => {
                    // Not finished, continue in next dispatch call.
                    let left = len - appended;
                    let data = data.wrapping_add(appended);
                    let continuation = KernelTask::WriteTx {
                        args,
                        total,
                        left,
                        data,
                        task,
                    };
                    Some(continuation)
                }
            }
        }
        KernelTask::Spawn { f, stack, stack_size } => {
            let stack = unsafe { core::slice::from_raw_parts_mut(stack, stack_size) };
            let task = Task::new(stack, f);
            insert_task(task);
            None
        }
        KernelTask::Unblock(task) => unsafe {
            (*task).set_blocked(false);
            None
        }
    }
}

/// Find a non-blocked Task in a list of Tasks.
fn find_runnable(mut head: *mut Task) -> Option<*mut Task> {
    unsafe {
        while !head.is_null() {
            if !(*head).is_blocked() {
                return Some(head);
            } else {
                head = (*head).next();
            }
        }
    }
    None
}

pub(crate) fn schedule_next() {
    let next = unsafe {
        // First, try to find a runnable Task after the current one.
        if let Some(next) = find_runnable((*CURRENT_TASK).next()) {
            next
        } else if let Some(any) = find_runnable(TASK_LIST) {
            any
        } else {
            get_idle_task_ptr()
        }
    };

    unsafe {
        CURRENT_TASK = next;
    }
}

/// Work through the queued up tasks and append any new ones for the next call to complete_tasks.
pub(crate) fn complete_tasks() {
    // Hold continued tasks in another queue to avoid blocking the dispatcher.
    let mut continuations: Queue<KernelTask, KERNEL_TASK_COUNT> = Queue::new();
    while let Some(task) = unsafe { KERNEL_TASK_QUEUE.dequeue() } {
        if let Some(new_task) = execute_task(task) {
            continuations.enqueue(new_task).expect("cannot create more tasks in dispatcher");
        }
    }

    // Swap out old (empty) task queue with new one.
    unsafe { KERNEL_TASK_QUEUE = continuations }
}