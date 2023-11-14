#![no_std]
#![no_main]
#![feature(
const_maybe_uninit_uninit_array,
maybe_uninit_uninit_array,
const_maybe_uninit_array_assume_init,
maybe_uninit_array_assume_init,
naked_functions
)]

use core::mem::MaybeUninit;
use core::{fmt::Write, panic::PanicInfo};
use core::ptr::null;
use cortex_m::delay::Delay;
use cortex_m::interrupt::{CriticalSection, Mutex};
use cortex_m::peripheral::NVIC;

use cortex_m::register::control::{Control, Npriv, Spsel};
use cortex_m_rt::{entry, exception};
use stm32f4xx_hal::{gpio::{gpioa, Output, PushPull}, interrupt, pac::{self, USART2}, prelude::*, serial::{Config, Serial, Tx}};
use stm32f4xx_hal::pac::Interrupt;
use stm32f4xx_hal::rtc::Error::InvalidInputData;

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
    /// Create a new TaskTable without any tasks.
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

#[repr(C)]
struct Task {
    stack_pointer: *const (),
    handler: fn(*const ()) -> *const (),
    params: *const (),
    state: TaskState,
}

impl Task {
    /// Create a new task from the current calling context.
    /// **Note:** Do not call this twice! Data is invalid and must be written to before being
    /// red.
    fn from_context() -> Self {
        Self {
            stack_pointer: null(),
            handler: |_| &{ task_finished() } as _,
            params: 0 as _,
            state: TaskState::Active,
        }
    }
}

fn task_finished() {
    loop {
        // let uart = unsafe { UART.as_mut() }.unwrap();
        // writeln!(uart, "Task finished!").unwrap();
        // delay(100000);
    }
}

static mut OS_CURRENT_TASK: *mut Task = core::ptr::null_mut();
static mut OS_NEXT_TASK: *mut Task = core::ptr::null_mut();

#[naked]
#[no_mangle]
#[allow(non_snake_case)]
fn PendSV() {
    // let uart = unsafe { UART.as_mut() }.unwrap();
    // writeln!(uart, "Hello from PendSV\r").unwrap();
    // writeln!(uart, "{}\r", cortex_m::register::psp::read()).unwrap();
    unsafe {
        /*
//         core::arch::asm!(
//         "mrs r0, psp                     \n",
//         "isb                             \n",
//
//         // Get the location of the current TCB.
//         "ldr     r3, ={0}   \n",
//         "ldr     r2, [r3]                \n",
//
//         // Is the task using the FPU context?  If so, push high vfp registers.
//         // "tst r14, #0x10                  \n",
//         // "it eq                           \n",
//         // "vstmdbeq r0!, {s16-s31}         \n",
// //
//         "stmdb r0!, {r4-r11, r14}        \n", // Save the core registers.
//         "str r0, [r2]                    \n", /* Save the new top of stack into the first member of the TCB. */
// //
//         "stmdb sp!, {r0, r3}             \n",
//         "mov r0, %0                      \n",
//         "msr basepri, r0                 \n",
//         "dsb                             \n",
//         "isb                             \n",
//         "bl vTaskSwitchContext           \n",
//         "mov r0, #0                      \n",
//         "msr basepri, r0                 \n",
//         "ldmia sp!, {r0, r3}             \n",
// //
//         "ldr r1, [r3]                    \n", /* The first item in pxCurrentTCB is the task top of stack. */
//         "ldr r0, [r1]                    \n",
// //
//         "ldmia r0!, {r4-r11, r14}        \n", /* Pop the core registers. */
// //
//         "tst r14, #0x10                  \n", /* Is the task using the FPU context?  If so, pop the high vfp registers too. */
//         "it eq                           \n",
//         "vldmiaeq r0!, {s16-s31}         \n",
// //
//         "msr psp, r0                     \n",
//         "isb                             \n",
// //
//         "bx r14                          \n",
//         sym OS_CURRENT_TASK,
//         sym OS_NEXT_TASK,
//         options(noreturn)
//         );
*/
        core::arch::asm!(
        // 1. Save r4-r11
        "push {{r4-r11, r14}}",

        // 2. Save stack pointer to task control block
        "ldr r0, ={0}", // Load address of OS_CURRENT_TASK into r0
        "ldr r0, [r0]", // Load contents of OS_CURRENT_TASK into r0
        "str SP, [r0]", // Store stack pointer into TSB

        // 3. Load next stack pointer from next TSB
        "ldr r0, ={1}", // Load address of OS_NEXT_TASK into r0
        "ldr r0, [r0]", // Load contents of OS_NEXT_TASK into r0
        "ldr SP, [r0]", // Store stack pointer into TSB

        // 4. Restore r4-r11
        "pop {{r4-r11, r14}}",

        // 5. Return in thread mode
        // "ldr r14, =0xFFFFFFFD",
        "bx r14",
        sym OS_CURRENT_TASK,
        sym OS_NEXT_TASK,
        options(noreturn),
        )
    };
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
    let id = params as i32;
    loop {
        cortex_m::interrupt::free(|_| {
            let uart = unsafe { UART.as_mut() }.unwrap();
            writeln!(uart, "Hello from task {:?} with id {}\r", unsafe { OS_CURRENT_TASK }, id).unwrap();
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
    writeln!(uart, "\x1b[2J\x1b[H").unwrap();
    writeln!(uart, "Setting up tasks\r").unwrap();

    let main_task = Task::from_context();
    unsafe { TASK_TABLE.insert_task(main_task) };

    let uart = unsafe { UART.as_mut() }.unwrap();
    writeln!(uart, "Set up tasks\r").unwrap();

    let mut syst = cp.SYST;
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.enable_counter();
    syst.enable_interrupt();
    syst.set_reload(8_000_000);

    unsafe { OS_CURRENT_TASK = TASK_TABLE.current_task() };

    // What do these do?
    // This sets the process stack pointer to some magic value. Maybe we need to change our approach here.
    // When this line is executed and we return to thread mode + psp, we land in the hard fault handler.
    unsafe { cortex_m::register::psp::write((*OS_CURRENT_TASK).stack_pointer as u32 + 64) };

    // Set threads to unprivileged mode
    let mut control = cortex_m::register::control::read();
    control.set_npriv(Npriv::Unprivileged);
    // control.set_spsel(Spsel::Psp);
    unsafe { cortex_m::register::control::write(control) };

    // Flush caches, probably not needed but found in reference docs.
    cortex_m::asm::isb();

    // unsafe { ((*OS_CURRENT_TASK).handler)((*OS_CURRENT_TASK).params) };

    loop {
        writeln!(uart, "Main Task loop!");
        cortex_m::asm::wfi();
    }
}

// 1. Switch to user mode (PSP, unprivileged)
// 2. Start new tasks by generating stack frame
// 3. Document stack, conditions, etc.
// 4. Implement features

// Motivation (why betriebssystem für Lehre?
//     Stark genug auf Context Wechsel legen (share CPU, not memory)
//     x86 is too complex (bootloader, real mode, cache, I/O)
//     Build one tool that may be used for teaching
// Platform: STM32 -> einfach! (expect comments, no MMU, no classical OS, Zweck: CTX wechsel, Übersichtlich, einfach)
// Prozess eines CTX switch erklären
//     Timer, PendSV, was passiert, privilegien, Stack pointer, wie sehen TBCs aus, wie sieht der Stack aus?
// Demo!
// Planned work:
//     - Features
//     - Spawn tasks
//     - IPC / Mutex / Semaphore
//     - MPU for Kernel/User space
//     - Drivers for Devices with protection instead of direct hardware access
//     - Scheduling policies
//         - Priority Inversion solution (aging, inheritance)
//     - Realtime?
