//! Basic Input/Output System using USART2 and a FIFO buffer.

use core::fmt::{Error, Write};
use cortex_m::asm::bkpt;
use cortex_m::peripheral::NVIC;
use stm32f4xx_hal::pac::{Interrupt, USART2};
use stm32f4xx_hal::serial::{Event, Listen, Serial, Tx, TxISR};
use crate::ring_buffer::RingBuffer;
use stm32f4xx_hal::interrupt;
use stm32f4xx_hal::prelude::_embedded_hal_serial_Write;

static mut SERIAL: Option<Serial<USART2>> = None;
static mut BUFFER: RingBuffer<16> = RingBuffer::new();

pub fn initialize(mut serial: Serial<USART2>) {
    unsafe {
        serial.listen(Event::Txe);
        SERIAL = Some(serial);
        NVIC::unmask(Interrupt::USART2)
    }
}

pub struct BlockingOutput;

pub fn output() -> BlockingOutput {
    BlockingOutput {}
}

impl Write for BlockingOutput {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let buffer = s.as_bytes();
        let fifo = unsafe { &mut BUFFER };
        // Send every byte.
        for byte in buffer {
            // Attempt until byte has been sent.
            while true {
                // Require atomic access to buffer for modification.
                if cortex_m::interrupt::free(|_ct| {
                    fifo.push_back(*byte)
                }) {
                    break;
                }
            }
        }
        Ok(())
    }
}

pub unsafe fn get_raw() -> Option<&'static mut Serial<USART2>> {
    SERIAL.as_mut()
}

pub fn write(buffer: &[u8]) {
    let fifo = unsafe { &mut BUFFER };
    let mut count = 0;
    while count < buffer.len() {
        // Send single character.
        while !fifo.push_back(buffer[count]) {}
        count += 1;
    }
}


#[interrupt]
fn USART2() {
    let serial = unsafe { get_raw().unwrap() };
    if serial.is_tx_empty() {
        // Send next byte from buffer, if possible.
        if let Some(byte) = unsafe { BUFFER.pop_front() } {
            serial.write(byte).unwrap()
        }
    }
}
