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
// RingBuffer::default()
#![feature(const_trait_impl)]


use core::{fmt::Write, panic::PanicInfo};
use cortex_m::asm::delay;
use cortex_m::peripheral::scb::{SystemHandler};
use cortex_m_rt::{entry, exception};
use stm32f4xx_hal::{pac::{self}, prelude::*, serial::{Config}};
use stm32f4xx_hal::timer::SysEvent;
use task::{OS_CURRENT_TASK};
use crate::task::{schedule_next_task, start_scheduler};

mod dispatcher;
mod task;
mod global_peripherals;
mod syscalls;
mod bios;
mod fifo;


#[panic_handler]
unsafe fn panic_handler(info: &PanicInfo) -> ! {
    let mut output = bios::buffered_output();
    if let Some(location) = info.location() {
        writeln!(
            output,
            "panicked at {} - {}:{} with message '{}'\n",
            location.file(),
            location.line(),
            location.column(),
            info.message().unwrap()
        );
    }
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        writeln!(output, "{}\r", s).unwrap();
    }
    loop {
        cortex_m::asm::bkpt()
    }
}

#[exception]
fn SysTick() {
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
    let mut raw_serial = usart2.serial::<u8>(pins, config, &clocks).unwrap();

    writeln!(raw_serial, "Sysclock at {}, Hclock at {}", clocks.sysclk(), clocks.hclk()).unwrap();
    writeln!(raw_serial, "Initializing BIOS...").unwrap();
    bios::initialize(raw_serial);


    let mut output = bios::buffered_output();

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
    writeln!(output, "Exception priorities configured!").unwrap();

    // write!(buffered, "Starting SysTick Timer...").unwrap();
    // let mut systick = cp.SYST.counter_hz(&clocks);
    // systick.listen(SysEvent::Update);
    // systick.start(1.Hz()).unwrap();
    // writeln!(buffered, "Done!").unwrap();


    let led_pin = gpioa.pa5.into_push_pull_output();
    unsafe { global_peripherals::LED = Some(led_pin); }

    writeln!(output, "Starting scheduler...").unwrap();
    unsafe { start_scheduler(&mut APPLICATION_STACK, app) }
}


/// Size of application stack in words (4 bytes).
const APP_STACK_SIZE: usize = 1280usize;
/// Application stack used after switch to scheduler.
static mut APPLICATION_STACK: [u32; APP_STACK_SIZE] = [0u32; APP_STACK_SIZE];

fn send_blocking(message: &str) -> Result<(), syscalls::SyscallError> {
    let buffer = message.as_bytes();
    let mut count = 0;
    while count < buffer.len() {
        let to_send = &buffer[count..];
        match syscalls::stubs::write(to_send) {
            Ok(c) => count += c,
            Err(syscalls::SyscallError::InsufficientSpace(c)) => count += c,
            Err(other) => { return Err(other); }
        }
    };
    Ok(())
}

fn app() -> ! {
    let very_long_message = include_str!("main.rs");
    loop {
        send_blocking(very_long_message).expect("sending failed");
        delay(8_000_000);
    }
}
