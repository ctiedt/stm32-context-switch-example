#![no_std]
#![no_main]
#![feature(
const_maybe_uninit_uninit_array,
maybe_uninit_uninit_array,
const_maybe_uninit_array_assume_init,
maybe_uninit_array_assume_init,
naked_functions
)]

use core::{fmt::Write, panic::PanicInfo};
use cortex_m::register::control::Npriv;
use cortex_m_rt::{entry, exception};
use stm32f4xx_hal::{gpio::{gpioa, Output, PushPull}, pac::{self, USART2}, prelude::*, serial::{Config, Serial, Tx}};
use task::{OS_CURRENT_TASK, OS_NEXT_TASK, Task, TASK_TABLE, TaskState};

mod dispatcher;
mod task;

static mut LED: Option<gpioa::PA5<Output<PushPull>>> = None;
static mut UART: Option<Tx<USART2>> = None;

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

fn task_finished() {
    loop {
        // let uart = unsafe { UART.as_mut() }.unwrap();
        // writeln!(uart, "Task finished!").unwrap();
        // delay(100000);
    }
}

#[exception]
fn SysTick() {
    unsafe { OS_CURRENT_TASK = TASK_TABLE.current_task() };
    unsafe { OS_NEXT_TASK = TASK_TABLE.next_task() };

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
    todo!("Switch to process stack");
    // unsafe { cortex_m::register::psp::write((*OS_CURRENT_TASK).stack_pointer as u32 + 64) };

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
