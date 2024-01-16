//! Basic Input/Output System using USART2 and a FIFO buffer.

use core::fmt::{Error, Write};
use cortex_m::asm::bkpt;
use cortex_m::peripheral::NVIC;
use stm32f4xx_hal::pac::{Interrupt, USART2};
use stm32f4xx_hal::serial::{Event, Listen, Serial, Tx, TxISR};
use crate::fifo::FIFO;
use stm32f4xx_hal::interrupt;
use stm32f4xx_hal::prelude::_embedded_hal_serial_Write;

static mut SERIAL: Option<Serial<USART2>> = None;
static mut BUFFER: FIFO<u8, 1280> = FIFO::new_with(0u8);

pub fn initialize(mut serial: Serial<USART2>) {
    unsafe {
        serial.listen(Event::Txe);
        SERIAL = Some(serial);
        NVIC::unmask(Interrupt::USART2)
    }
}

/// Raw, unbuffered access to output.
pub struct RawOutput;

pub fn raw_output() -> RawOutput {
    RawOutput {}
}

impl Write for RawOutput {
    fn write_str(&mut self, buffer: &str) -> core::fmt::Result {
        // Ignore any buffered output and write out data.
        // To do this, we disable all interrupts.
        cortex_m::interrupt::free(|_| unsafe {
            let serial = get_raw().unwrap();
            let result = serial.write_str(buffer);
            result
        })
    }
}

fn get_raw() -> Option<&'static mut Serial<USART2>> {
    unsafe { SERIAL.as_mut() }
}


#[interrupt]
fn USART2() {
    unsafe { send_next_byte() }
}

/// Helper function to send a single byte of data, if possible.
unsafe fn send_next_byte() {
    let serial = get_raw().unwrap();
    let fifo = &mut BUFFER;
    if !serial.is_tx_empty() {
        return;
    }
    if let Some(byte) = fifo.pop_front() {
        serial.write(byte).unwrap();
    }
}