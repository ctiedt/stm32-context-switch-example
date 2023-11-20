use core::fmt::Write;
use stm32f4xx_hal::gpio::{gpioa, Output, Pin, PushPull};
use stm32f4xx_hal::pac::{USART2};
use stm32f4xx_hal::serial::Serial;

pub(crate) static mut LED: Option<gpioa::PA5<Output<PushPull>>> = None;
pub(crate) static mut UART: Option<Serial<USART2>> = None;

pub(crate) static mut SYNC_SERIAL2: SyncSerial = SyncSerial {};

pub(crate) struct SyncSerial {}

impl Write for SyncSerial {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        // cortex_m::interrupt::disable();
        // cortex_m::interrupt::free(|cs| {
        let uart = unsafe { UART.as_mut() }.unwrap();
        write!(uart, "{}", s)
        // })
        // unsafe { cortex_m::interrupt::enable(); }
    }
}
