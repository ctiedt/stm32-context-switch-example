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

use core::{fmt::Write, panic::PanicInfo};
use core::ptr::null;
use cortex_m::peripheral::scb::{Exception, SystemHandler};
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::register::control::Npriv;
use cortex_m_rt::{entry, exception};
use stm32f4xx_hal::{gpio::{gpioa, Output, PushPull}, pac::{self, USART2}, prelude::*, serial::{Config, Serial, Tx}};
use stm32f4xx_hal::gpio::gpioa::Parts;
use stm32f4xx_hal::pac::Interrupt;
use stm32f4xx_hal::rcc::Clocks;
use stm32f4xx_hal::serial::config::Parity;
use stm32f4xx_hal::serial::{Rx, Serial2};
use task::{OS_CURRENT_TASK, OS_NEXT_TASK, Task, TASK_TABLE};
use crate::global_peripherals::UART;
use crate::task::{create_task, initialize_scheduler, schedule_next_task};

mod dispatcher;
mod task;
mod global_peripherals;


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

fn task_finished() {
    loop {
        // let uart = unsafe { UART.as_mut() }.unwrap();
        // writeln!(uart, "Task finished!").unwrap();
        // delay(100000);
    }
}

#[exception]
fn SysTick() {
    let serial2 = unsafe { &mut global_peripherals::SYNC_SERIAL2 };
    // writeln!(serial2, "Tick!").unwrap();
    schedule_next_task();
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
            let uart = unsafe { global_peripherals::UART.as_mut() }.unwrap();
            writeln!(uart, "Hello from task {:?} with id {}\r", unsafe { OS_CURRENT_TASK }, id).unwrap();
            let led = unsafe { global_peripherals::LED.as_mut() }.unwrap();
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
    let mut serial2 = usart2.serial::<u8>(pins, config, &clocks).unwrap();

    unsafe { global_peripherals::UART = Some(serial2); }

    let mut led_pin = gpioa.pa5.into_push_pull_output();
    unsafe { global_peripherals::LED = Some(led_pin); }

    let serial2 = unsafe { &mut global_peripherals::SYNC_SERIAL2 };
    writeln!(serial2, "Hello, world!").unwrap();
    writeln!(serial2, "Core clock is at {} Hz", clocks.hclk()).unwrap();
    writeln!(serial2, "Clocks: {:?}", clocks).unwrap();

    // todo!("Setup kernel space memory protection");
    // todo!("Setup interrupt priorities");

    let mut scb = cp.SCB;
    unsafe {
        scb.set_priority(SystemHandler::PendSV, 15);
        scb.set_priority(SystemHandler::SVCall, 14);
        scb.set_priority(SystemHandler::SysTick, 13);
    }
    writeln!(serial2, "NVIC configured!").unwrap();

    initialize_scheduler();
    writeln!(serial2, "Scheduler initialized!").unwrap();

    unsafe { create_task(some_task, null(), &mut SOME_TASK_STACK); }
    writeln!(serial2, "Second task created!").unwrap();

    let mut SYST = cp.SYST;
    SYST.set_clock_source(SystClkSource::Core);
    SYST.set_reload(8_000_000);
    SYST.enable_counter();
    SYST.enable_interrupt();
    writeln!(serial2, "SysTick enabled!").unwrap();
    //
    // writeln!(serial2, "Curent task: {:?} Next: {:?}", unsafe { OS_CURRENT_TASK }, unsafe { OS_NEXT_TASK }).unwrap();
    // schedule_next_task();
    // writeln!(serial2, "Curent task: {:?} Next: {:?}", unsafe { OS_CURRENT_TASK }, unsafe { OS_NEXT_TASK }).unwrap();
    // schedule_next_task();
    // writeln!(serial2, "Curent task: {:?} Next: {:?}", unsafe { OS_CURRENT_TASK }, unsafe { OS_NEXT_TASK }).unwrap();


    // Hey Philipp! Here you need to start thinking about global state. Maybe you should store references to the UART in a place where the panic handler can access it?


    loop {
        writeln!(serial2, "Loop!").unwrap();
        cortex_m::asm::wfi();
    }
    todo!("Load 'thread mode, unprivileged' into Link Register");
    todo!("Switch to scheduled mode");
}

static mut SOME_TASK_STACK: [u32; 256] = [0u32; 256];

fn some_task() {
    unsafe {
        let led = global_peripherals::LED.as_mut().unwrap();
        loop {
            led.toggle();
            cortex_m::asm::wfi();
        }
    }
}