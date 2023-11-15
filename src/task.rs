use core::ptr::null;
use core::mem::MaybeUninit;

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

    pub fn next_task(&mut self) -> &mut Task {
        self.current = (self.current + 1) % self.size;
        unsafe { self.tasks[self.current].assume_init_mut() }
    }
}

#[repr(u8)]
pub(crate) enum TaskState {
    Idle,
    Active,
}

#[repr(C)]
pub(crate) struct Task {
    stack_pointer: *const (),
    handler: fn(*const ()) -> *const (),
    params: *const (),
    state: TaskState,
}

impl Task {
    /// Create a new task from the current calling context.
    /// **Note:** Do not call this twice! Data is invalid and must be written to before being
    /// red.
    pub fn from_context() -> Self {
        Self {
            stack_pointer: null(),
            handler: |_| &{ crate::task_finished() } as _,
            params: 0 as _,
            state: TaskState::Active,
        }
    }
}

pub(crate) static mut OS_CURRENT_TASK: *mut Task = core::ptr::null_mut();
pub(crate) static mut OS_NEXT_TASK: *mut Task = core::ptr::null_mut();
