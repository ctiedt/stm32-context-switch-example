use cortex_m_rt::{exception, ExceptionFrame};
use crate::task::{OS_CURRENT_TASK, OS_NEXT_TASK};
use core::fmt::Write;
use cortex_m::asm::bkpt;
use crate::bios;

#[naked]
#[no_mangle]
#[allow(non_snake_case)]
fn PendSV() {
    unsafe {
        core::arch::asm!(
        // 1. Save r4-r11
        "push {{r4-r11}}",

        // 2. Save stack pointer to task control block
        "ldr r0, ={0}", // Load address of OS_CURRENT_TASK into r0
        "ldr r0, [r0]", // Load contents of OS_CURRENT_TASK into r0
        "str SP, [r0]", // Store stack pointer into TSB

        // 3. Load next stack pointer from next TSB
        "ldr r0, ={1}", // Load address of OS_NEXT_TASK into r0
        "ldr r0, [r0]", // Load contents of OS_NEXT_TASK into r0
        "ldr SP, [r0]", // Store stack pointer into TSB

        // 3.1 Force Cache Flush? After stack change.
        "isb",
        "dsb",

        // 4. Restore r4-r11
        "pop {{r4-r11}}",

        // 5. Return to mode we came from.
        "bx lr",
        sym OS_CURRENT_TASK,
        sym OS_NEXT_TASK,
        options(noreturn),
        )
    };
}


#[exception]
unsafe fn HardFault(frame: &ExceptionFrame) -> ! {
    let core_peripherals = cortex_m::peripheral::Peripherals::steal();
    let cfsr = core_peripherals.SCB.cfsr.read();
    let usage_fault = (cfsr >> 16) as u16;
    let bus_fault = ((cfsr >> 8) & 0xff) as u8;
    let memory_fault = cfsr as u8;


    let mut serial = bios::take_serial().expect("cannot print before serial is configured");
    writeln!(serial, "Hard Fault {:?}", frame).unwrap();
    writeln!(serial, "UFSR={:#016b}", usage_fault).unwrap();
    writeln!(serial, "BFSR={:#08b}", bus_fault).unwrap();
    writeln!(serial, "MMFSR={:#08b}", memory_fault).unwrap();

    // Recovery is highly unlikely, so we simply wait for a manual reset and allow debugging.
    loop { bkpt() }
}
