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

/// Pointers to previous (switched from) and current (switched to) Tasks.
pub(super) static mut PREVIOUS_TASK: *mut Task = core::ptr::null_mut();
pub(super) static mut NEXT_TASK: *mut Task = core::ptr::null_mut();


/// Idle [Task] used when no others can run.
static mut IDLE_TASK: Option<Task> = None;


/// Get idle Task pointer. Requires [IDLE_TASK] to be [Some].
unsafe fn get_idle_task_ptr() -> *mut Task {
    IDLE_TASK.as_mut().expect("idle task was not initialized")
}

/// Ring-shaped linked list of all available tasks.
static mut TASK_LIST: *mut Task = core::ptr::null_mut();

/// Insert a new Task into the list of available Tasks.
fn insert_task(task: *mut Task) {
    unsafe {
        (*task).link_to(TASK_LIST);
        TASK_LIST = task;
    }
}


pub fn start(clocks: &Clocks, syst: SYST, app_stack: &mut [u32], app: impl FnOnce() + 'static) -> ! {
    /// Create and save idle Task to switch to later.
    unsafe {
        let idle_task = Task::new_empty();
        IDLE_TASK = Some(idle_task);
    }

    /// Create application Task and insert it into Task list.
    let app_task = Task::new(app_stack, app);
    let app_task_ptr = Box::into_raw(Box::new(app_task));
    insert_task(app_task_ptr);

    /// Set idle task as initial "switched from" task and app as "switched to" task to simulate a
    /// switch from a previously idle system.
    unsafe {
        PREVIOUS_TASK = get_idle_task_ptr();
        NEXT_TASK = app_task_ptr;
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
}

/// Attempt to enqueue a new kernel mode task.
/// Returns [Err] containing `task` if queue is full.
pub fn enqueue_task(task: KernelTask) -> Result<(), KernelTask> {
    unsafe { KERNEL_TASK_QUEUE.enqueue(task) }
}

/// Execute a kernel mode task, if available.
/// Returns `true` if a task was executed.
pub fn execute_task() -> bool {
    let task = match unsafe { KERNEL_TASK_QUEUE.dequeue() } {
        None => { return false; }
        Some(task) => task
    };

    match task {
        KernelTask::LowerThreadPrivileges => {
            let mut control = cortex_m::register::control::read();
            control.set_npriv(Npriv::Unprivileged);
            unsafe { cortex_m::register::control::write(control); }
        }
        KernelTask::WriteTx { args, total, left: len, data, task } => {
            let buffer = unsafe { core::slice::from_raw_parts(data, len) };
            let mut tx = bios::buffered_output();
            match tx.append(buffer) {
                Ok(_) => unsafe {
                    // Notify Task of success.
                    let args = core::slice::from_raw_parts_mut(args, 2);
                    args[0] = ReturnCode::Ok as u32;
                    args[1] = total as u32;
                    (*task).set_blocked(false);
                }
                Err(appended) => {
                    let left = len - appended;
                    let data = data.wrapping_add(appended);
                    let new_task = KernelTask::WriteTx {
                        args,
                        total,
                        left,
                        data,
                        task,
                    };
                    enqueue_task(new_task).expect("failed to continue write");
                }
            }
        }
    }
    true
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
        if let Some(next) = find_runnable((*NEXT_TASK).next()) {
            next
        } else if let Some(any) = find_runnable(TASK_LIST) {
            any
        } else {
            get_idle_task_ptr()
        }
    };

    unsafe {
        PREVIOUS_TASK = NEXT_TASK;
        NEXT_TASK = next;
    }
}
