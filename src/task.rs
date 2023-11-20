use core::ptr::{null, null_mut};
use core::mem::MaybeUninit;
use crate::global_peripherals;
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
    // handler: fn(*const ()) -> *const (),
    params: *const (),
}

impl Task {
    /// Create a new task from the current calling context.
    /// **Note:** Do not call this twice! Data is invalid and must be written to before being
    /// read.
    pub fn from_context() -> Self {
        Self {
            stack_pointer: null_mut(),
            // handler: |_| &{ unreachable!() } as _,
            params: 0 as _,
        }
    }
}

pub(crate) static mut OS_CURRENT_TASK: *mut Task = core::ptr::null_mut();
pub(crate) static mut OS_NEXT_TASK: *mut Task = core::ptr::null_mut();

pub(crate) fn initialize_scheduler() {
    unsafe {
        TASK_TABLE.insert_task(Task::from_context());
        OS_NEXT_TASK = TASK_TABLE.next_task().expect("failed to initialize current task");
        OS_CURRENT_TASK = OS_NEXT_TASK;
    };
}

pub(crate) fn schedule_next_task() {
    unsafe { OS_CURRENT_TASK = OS_NEXT_TASK; }
    unsafe { OS_NEXT_TASK = TASK_TABLE.next_task().expect("failed to get next task") };

    let serial2 = unsafe { &mut global_peripherals::SYNC_SERIAL2 };
    // writeln!(serial2, "Now running task {:?}\r", unsafe { OS_NEXT_TASK }).unwrap();
}

pub(crate) fn create_task(handler: fn() -> (), params: *const (), stack: &mut [u32]) {
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
    push!(0xFFFFFFF9u32);
    push!(11);
    push!(10);
    push!(9);
    push!(8);
    push!(7);
    push!(6);
    push!(5);
    push!(4);


    let task = Task { stack_pointer: top, params };
    unsafe { TASK_TABLE.insert_task(task); }
}

fn task_finished() {
    loop {
        cortex_m::asm::bkpt();
    }
}