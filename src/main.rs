#![no_std]
#![no_main]
#![feature(
    const_maybe_uninit_uninit_array,
    maybe_uninit_uninit_array,
    const_maybe_uninit_array_assume_init,
    maybe_uninit_array_assume_init
)]

use core::mem::MaybeUninit;
use core::{fmt::Write, panic::PanicInfo};

use cortex_m::register::control::Control;
use cortex_m_rt::{entry, exception};
use stm32f4xx_hal::{
    gpio::{gpioa, Output, PushPull},
    pac::{self, USART2},
    prelude::*,
    serial::{Config, Serial, Tx},
};

static mut LED: Option<gpioa::PA5<Output<PushPull>>> = None;
static mut UART: Option<Tx<USART2>> = None;

const MAX_TASKS: usize = 8;

static mut TASK_TABLE: TaskTable = TaskTable::new();

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    let uart = unsafe { UART.as_mut() }.unwrap();
    if let Some(location) = info.location() {
        writeln!(
            uart,
            "{} - {}:{}\r",
            location.file(),
            location.line(),
            location.column()
        )
        .unwrap();
    }
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        writeln!(uart, "{}\r", s).unwrap();
    }
    loop {}
}

struct TaskTable {
    tasks: [MaybeUninit<Task>; MAX_TASKS],
    current: usize,
    size: usize,
}

impl TaskTable {
    const fn new() -> Self {
        let tasks = MaybeUninit::uninit_array();
        Self {
            tasks,
            current: 0,
            size: 0,
        }
    }

    fn insert_task(&mut self, task: Task) {
        unsafe { *self.tasks[self.size].assume_init_mut() = task };
        self.size += 1;
    }

    fn current_task(&mut self) -> &mut Task {
        unsafe { self.tasks[self.current].assume_init_mut() }
    }

    fn next_task(&mut self) -> &mut Task {
        self.current = (self.current + 1) % self.size;
        unsafe { self.tasks[self.current].assume_init_mut() }
    }
}

#[repr(u8)]
enum TaskState {
    Idle,
    Active,
}

struct Task {
    stack_pointer: *const (),
    handler: fn(*const ()) -> *const (),
    params: *const (),
    state: TaskState,
}

fn os_task_init(
    handler: fn(*const ()) -> *const (),
    params: *const (),
    stack: *mut (),
    stack_size: usize,
) {
    let stack_offset = stack_size / core::mem::size_of::<usize>();

    let task = Task {
        stack_pointer: unsafe { (stack as *const u8).add(stack_offset - 16) as *const () },
        handler,
        params,
        state: TaskState::Idle,
    };

    unsafe { TASK_TABLE.insert_task(task) };

    unsafe {
        stack
            .cast::<usize>()
            .add(stack_offset - 1)
            .write(0x01000000);

        stack
            .cast::<usize>()
            .add(stack_offset - 2)
            .write(handler as usize & !0x01);

        stack
            .cast::<usize>()
            .add(stack_offset - 3)
            .write(task_finished as _);

        stack
            .cast::<usize>()
            .add(stack_offset - 8)
            .write(params as _);
    };
}

fn task_finished() {
    loop {}
}

static mut OS_CURRENT_TASK: *mut Task = core::ptr::null_mut();
static mut OS_NEXT_TASK: *mut Task = core::ptr::null_mut();

#[exception]
fn PendSV() {
    let uart = unsafe { UART.as_mut() }.unwrap();
    writeln!(uart, "Hello from PendSV\r").unwrap();
    writeln!(uart, "{}\r", cortex_m::register::psp::read()).unwrap();

    cortex_m::interrupt::free(|_| unsafe {
        core::arch::asm!(
            // Save registers R4-R11
            "mrs r0, psp",
            "subs r0, #16",
            "stmia r0!, {{r4-r7}}",
            "mov r4, r8",
            "mov r5, r9",
            "mov r6, r10",
            "mov r7, r11",
            "subs r0, #32",
            "stmia r0!, {{r4-r7}}",
            "subs r0, #16",
            // Save current task's sp
            "ldr r2, ={0}",
            "ldr r1, [r2]",
            "str r0, [r1]",
            // Load next task's sp
            "ldr r2, ={1}",
            "ldr r1, [r2]",
            "ldr r0, [r1]",
            // Load registers R4-R11 for new task
            "ldmia r0!, {{r4-r7}}",
            "mov r8, r4",
            "mov r9, r5",
            "mov r10, r6",
            "mov r11, r7",
            "ldmia r0!, {{r4-r7}}",
            "msr psp, r0",
            // Return from ISR
            "ldr r0, =0xFFFFFFFD",
            "bx r0",
            sym OS_CURRENT_TASK,
            sym OS_NEXT_TASK
        )
    });
}

#[exception]
fn SysTick() {
    unsafe { OS_CURRENT_TASK = TASK_TABLE.current_task() };
    unsafe { (*OS_CURRENT_TASK).state = TaskState::Idle };

    unsafe { OS_NEXT_TASK = TASK_TABLE.next_task() };
    unsafe { (*OS_NEXT_TASK).state = TaskState::Active };

    let uart = unsafe { UART.as_mut() }.unwrap();
    writeln!(uart, "Now running task {:?}\r", unsafe { OS_NEXT_TASK }).unwrap();

    cortex_m::peripheral::SCB::set_pendsv();
}

fn delay(mut time: u32) {
    while time > 0 {
        time -= 1;
    }
}

fn task_handler(params: *const ()) -> *const () {
    loop {
        cortex_m::interrupt::free(|_| {
            let uart = unsafe { UART.as_mut() }.unwrap();
            writeln!(uart, "Hello from task {:?}\r", unsafe { OS_CURRENT_TASK }).unwrap();
            let led = unsafe { LED.as_mut() }.unwrap();
            led.toggle();
        });

        delay(params as u32);
    }
}

#[entry]
fn main() -> ! {
    let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    let rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.use_hse(8.MHz()).freeze();

    let gpioa = dp.GPIOA.split();
    unsafe { LED.replace(gpioa.pa5.into_push_pull_output()) };

    let uart = dp.USART2;
    let tx = Serial::tx(
        uart,
        gpioa.pa2.into_alternate(),
        Config::default().baudrate(9600.bps()),
        &clocks,
    )
    .unwrap();
    unsafe { UART.replace(tx) };

    let uart = unsafe { UART.as_mut() }.unwrap();
    writeln!(uart, "Setting up tasks\r").unwrap();

    let stack0 = [0u32; 128];
    let stack1 = [0u32; 128];
    let stack2 = [0u32; 128];

    let p0 = 200000;
    let p1 = p0 / 2;
    let p2 = p0 / 4;

    os_task_init(
        task_handler as _,
        p0 as _,
        stack0.as_ptr() as _,
        core::mem::size_of_val(&stack0),
    );
    os_task_init(
        task_handler as _,
        p1 as _,
        stack1.as_ptr() as _,
        core::mem::size_of_val(&stack0),
    );
    os_task_init(
        task_handler as _,
        p2 as _,
        stack2.as_ptr() as _,
        core::mem::size_of_val(&stack0),
    );

    let uart = unsafe { UART.as_mut() }.unwrap();
    writeln!(uart, "Set up tasks\r").unwrap();

    let mut syst = cp.SYST;
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.enable_counter();
    syst.enable_interrupt();
    syst.set_reload(8_000_000);

    unsafe { OS_CURRENT_TASK = TASK_TABLE.current_task() };

    unsafe { cortex_m::register::psp::write((*OS_CURRENT_TASK).stack_pointer as u32 + 64) };
    unsafe { cortex_m::register::control::write(Control::from_bits(0x1)) };
    cortex_m::asm::isb();

    unsafe { ((*OS_CURRENT_TASK).handler)((*OS_CURRENT_TASK).params) };

    loop {
        cortex_m::asm::wfi();
    }
}
