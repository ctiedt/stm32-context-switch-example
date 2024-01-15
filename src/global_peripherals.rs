use core::fmt::Write;
use stm32f4xx_hal::gpio::{gpioa, Output, PushPull};
use stm32f4xx_hal::pac::{USART2};
use stm32f4xx_hal::serial::Serial;

pub(crate) static mut LED: Option<gpioa::PA5<Output<PushPull>>> = None;
