#![no_std]
#![no_main]
#![feature(
const_maybe_uninit_uninit_array,
maybe_uninit_uninit_array,
const_maybe_uninit_array_assume_init,
maybe_uninit_array_assume_init,
naked_functions
)]
#![feature(ascii_char)]
#![feature(panic_info_message)]
#![feature(asm_const)]
#![feature(never_type)]

use core::{fmt::Write, panic::PanicInfo};
use cortex_m::peripheral::scb::{SystemHandler};
use cortex_m_rt::{entry, exception};
use stm32f4xx_hal::{pac::{self}, prelude::*, serial::{Config}};
use task::{OS_CURRENT_TASK};
use crate::task::{schedule_next_task, start_scheduler};

mod dispatcher;
mod task;
mod global_peripherals;
mod syscalls;


#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    let uart = unsafe { global_peripherals::UART.as_mut() }.unwrap();
    if let Some(location) = info.location() {
        writeln!(
            uart,
            "panicked at {} - {}:{} with message '{}'\n",
            location.file(),
            location.line(),
            location.column(),
            info.message().unwrap()
        )
            .unwrap();
    }
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        writeln!(uart, "{}\r", s).unwrap();
    }
    loop {}
}

#[exception]
fn SysTick() {
    let serial2 = unsafe { &mut global_peripherals::SYNC_SERIAL2 };
    writeln!(serial2, "Tick!").unwrap();
    schedule_next_task();
    cortex_m::peripheral::SCB::set_pendsv();
}

#[entry]
fn main() -> ! {
    let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    let rcc = dp.RCC.constrain();
    // ST-LINK Chip provides 8Mhz clock in default configuration
    let clocks = rcc.cfgr
        .use_hse(8.MHz())
        .freeze();

    let gpioa = dp.GPIOA.split();
    let tx2_pin = gpioa.pa2.into_alternate();
    let rx2_pin = gpioa.pa3.into_alternate();
    let usart2 = dp.USART2;
    let config = Config::default().baudrate(9600.bps());
    let pins = (tx2_pin, rx2_pin);
    let serial2 = usart2.serial::<u8>(pins, config, &clocks).unwrap();

    unsafe { global_peripherals::UART = Some(serial2); }

    let led_pin = gpioa.pa5.into_push_pull_output();
    unsafe { global_peripherals::LED = Some(led_pin); }

    let serial2 = unsafe { &mut global_peripherals::SYNC_SERIAL2 };
    writeln!(serial2, "Hello, world!").unwrap();
    writeln!(serial2, "Core clock is at {} Hz", clocks.hclk()).unwrap();
    writeln!(serial2, "Clocks: {:?}", clocks).unwrap();

    // todo!("Setup kernel space memory protection");
    // todo!("Setup interrupt priorities");

    /// Set priority levels of core exceptions. Lower number => higher priority.
    let mut scb = cp.SCB;
    unsafe {
        /// PendSV must have lowest priority to allow SysTick and SVCall to interrupt it.
        /// This way, the dispatcher (running in PendSV and performing a context switch) can be certain
        /// that it it not interrupting another exception or interrupt and corrupt its stack.
        /// Otherwise, we could switch away during an interrupt and block it until we switch back.
        scb.set_priority(SystemHandler::PendSV, 15);
        /// SysTick is next, it fires periodically and schedules the next task and requests
        /// PendSV to run after it returns.
        scb.set_priority(SystemHandler::SysTick, 14);
        /// Finally, SVCall. It serves as a ways to enter kernel mode and make system calls.
        /// Its priority is higher than SysTick so that the currently active task (or the next) does
        /// not change during handling of a system call.
        scb.set_priority(SystemHandler::SVCall, 13);
    }
    writeln!(serial2, "Exception priorities configured!").unwrap();

    writeln!(serial2, "Starting scheduler...").unwrap();
    unsafe { start_scheduler(&mut APPLICATION_STACK, app) }
}


/// Size of application stack in words (4 bytes).
const APP_STACK_SIZE: usize = 1280usize;
/// Application stack used after switch to scheduler.
static mut APPLICATION_STACK: [u32; APP_STACK_SIZE] = [0u32; APP_STACK_SIZE];

fn app() -> ! {
    let serial2 = unsafe { &mut global_peripherals::SYNC_SERIAL2 };
    writeln!(serial2, "Hello from App!").unwrap();
    let mut i = 0;
    loop {
        writeln!(serial2, "i={}", i).unwrap();
        i = syscalls::stub::increment(i).unwrap();
    }
}