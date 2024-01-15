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
static mut BUFFER: RingBuffer<1280> = RingBuffer::new();

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
            while !cortex_m::interrupt::free(|_| unsafe {
                let ok = BUFFER.push_back(*byte);
                if !ok {
                    cortex_m::asm::delay(8000);
                }
                ok
            }) {}
        }
        Ok(())
    }
}

pub unsafe fn get_raw() -> Option<&'static mut Serial<USART2>> {
    SERIAL.as_mut()
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