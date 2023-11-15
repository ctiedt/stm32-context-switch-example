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

use core::{fmt::Write, panic::PanicInfo};
use cortex_m::register::control::Npriv;
use cortex_m_rt::{entry, exception};
use stm32f4xx_hal::{gpio::{gpioa, Output, PushPull}, pac::{self, USART2}, prelude::*, serial::{Config, Serial, Tx}};
use stm32f4xx_hal::gpio::gpioa::Parts;
use stm32f4xx_hal::rcc::Clocks;
use stm32f4xx_hal::serial::config::Parity;
use stm32f4xx_hal::serial::Rx;
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
    let clocks = rcc.cfgr.freeze();


    let gpioa = dp.GPIOA.split();
    let tx2_pin = gpioa.pa2.into_alternate();
    let rx2_pin = gpioa.pa3.into_alternate();
    let usart2 = dp.USART2;
    let config = Config::default().baudrate(9600.bps());
    let pins = (tx2_pin, rx2_pin);
    let mut serial2 = usart2.serial::<u8>(pins, config, &clocks).unwrap();

    let mut led_pin = gpioa.pa5.into_push_pull_output();

    // todo!("Setup kernel space memory protection");
    // todo!("Setup interrupt priorities");

    // Hey Philipp! Here you need to start thinking about global state. Maybe you should store references to the UART in a place where the panic handler can access it?

    todo!("Create new 'main' thread");
    todo!("Load 'thread mode, unprivileged' into Link Register");
    todo!("Switch to scheduled mode");
}
