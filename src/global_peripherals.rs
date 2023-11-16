use stm32f4xx_hal::gpio::{gpioa, Output, Pin, PushPull};
use stm32f4xx_hal::pac::{USART2};
use stm32f4xx_hal::serial::Serial;

pub(crate) static mut LED: Option<gpioa::PA5<Output<PushPull>>> = None;
pub(crate) static mut UART: Option<Serial<USART2>> = None;