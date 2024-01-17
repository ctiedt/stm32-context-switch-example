//! Basic Input/Output System using USART2 and a FIFO buffer.

use core::fmt::{Error, Write};
use cortex_m::asm::bkpt;
use cortex_m::peripheral::NVIC;
use stm32f4xx_hal::pac::{Interrupt, USART2};
use stm32f4xx_hal::serial::{CFlag, Flag};
use stm32f4xx_hal::uart::{TxISR, Event};
use stm32f4xx_hal::{ClearFlags, interrupt, ReadFlags};
use stm32f4xx_hal::hal_02::serial::Write as W;
use stm32f4xx_hal::Listen;

type FIFO = crate::fifo::FIFO<u8, 1280>;
type Serial = stm32f4xx_hal::serial::Serial<USART2>;

static mut SERIAL: Option<Serial> = None;
static mut TX_BUFFER: FIFO = FIFO::new_with(0u8);

pub fn initialize(mut serial: Serial) {
    unsafe {
        // Wait for completion of any previous transmissions before enabling interrupt.
        while !serial.is_tx_empty() {}

        // Clear pending flag to not trigger immediately and enable interrupt.
        NVIC::unpend(Interrupt::USART2);
        NVIC::unmask(Interrupt::USART2);

        // Hand over to BIOS and start listening;
        SERIAL = Some(serial);
        enable_tx_interrupt();
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
        // To do this, we disable all USART2 interrupts.
        unsafe {
            let interrupt_enabled = NVIC::is_enabled(Interrupt::USART2);
            NVIC::mask(Interrupt::USART2);

            let tx = get_raw_serial();
            // Wait for most recent transmission to complete.
            while !tx.is_tx_empty() {}
            let result = tx.write_str(buffer);
            // Wait for our transmission to complete.
            while !tx.is_tx_empty() {}

            // Continue interrupt-driven transmission and re-enable interrupt.
            send_next(tx);
            if interrupt_enabled { NVIC::unmask(Interrupt::USART2) }
            result
        }
    }
}

/// Buffered output with interrupt.
pub struct BufferedOutput;

pub fn buffered_output() -> BufferedOutput { BufferedOutput }

impl Write for BufferedOutput {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        unsafe {
            let tx = get_raw_serial();
            let fifo = get_raw_tx_buffer();

            // We need to disable our interrupt to safely access the queue.
            disable_tx_interrupt();
            // Await last transmission.
            while !tx.is_tx_empty() {}
            for character in string.bytes() {
                if !fifo.push_back(character) {
                    break;
                }
            }

            // Start transmission and re-enable interrupt.
            send_next(tx);
            enable_tx_interrupt();
            Ok(())
        }
    }
}

unsafe fn get_raw_serial() -> &'static mut Serial {
    SERIAL.as_mut().unwrap()
}

unsafe fn get_raw_tx_buffer() -> &'static mut FIFO {
    &mut TX_BUFFER
}


/// Helper function to disable the transmission interrupt before a critical section.
unsafe fn disable_tx_interrupt() {
    let serial = get_raw_serial();
    serial.unlisten(Event::TransmissionComplete);
}

/// Helper function to enable the transmission interrupt after a critical section.
unsafe fn enable_tx_interrupt() {
    let serial = get_raw_serial();
    serial.listen(Event::TransmissionComplete);
}

#[interrupt]
unsafe fn USART2() {
    let tx = get_raw_serial();
    if tx.is_tx_empty() {
        let fifo = get_raw_tx_buffer();
        if let Some(byte) = fifo.pop_front() {
            // Writing new data clears the transmission complete interrupt.
            tx.write(byte).unwrap();
        } else {
            // Clear transmission complete flag to stop interrupt from triggering.
            tx.clear_flags(CFlag::TransmissionComplete);
        }
    }
}

/// Helper function to start a transmission or simple write byte.
fn send_next(tx: &mut Serial) {
    let fifo = unsafe { get_raw_tx_buffer() };
    if let Some(byte) = fifo.pop_front() {
        tx.write(byte).unwrap();
    }
}
