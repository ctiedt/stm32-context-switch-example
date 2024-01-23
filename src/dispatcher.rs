use cortex_m_rt::{exception, ExceptionFrame};
use core::fmt::Write;
use cortex_m::asm::bkpt;
use crate::{bios, scheduler};

#[naked]
#[no_mangle]
#[allow(non_snake_case)]
/// Sequence of a context switch
/// 1. Save all registers to stack we came from
/// 2. Call kernel_task
/// 3. Restore registers from next active thread
/// 4. Return to (potentially different) thread.
fn PendSV() {
    unsafe {
        core::arch::asm!(
        // We store MSP to r0 and PSP to r1 for later use.
        "mrs r0, MSP",
        "mrs r1, PSP",
        // Instruction Synchronization Barrier, necessary according to ARM.
        "isb",

        // 1. Save all registers to stack we came from.
        // We need to store to MSP or PSP depending on bit 2 of LR.
        // If it is set, we are using PSP, otherwise we are using MSP.
        "tst LR, #(1<<2)",
        // Z flag (eq) is set if bit was *not* set.
        "ite eq",
        // If "equal" (eq set, bit 2 not set), we increment SP to make space for saved registers.
        // We save r4-r11 and LR, 9 registers in total, each 4 bytes in size.
        "subeq SP, SP, (4 * 9)",
        // Otherwise, we move PSP into r0, since we will use that to push our registers.
        "movne r0, r1",
        // ISB here to synchronize after write to SP.
        "isb",

        // Instruction synchronization barrier to

        // Now, we use r0 as index and Store Multiple while Decrementing Before and write the address
        // of the last stored back into r0. This is basically push with r0 instead of SP.
        // Order must match with later loading of those registers as well as stack initialization
        // for new threads.
        "stmdb r0!, {{r4-r11, LR}}",

        // Finally, we store the new top of stack into the first member of the previous (switched from)
        // thread by loading the location of the pointer to it, dereferencing it to obtain the location
        // of the thread and storing into the first word at that location.
        "ldr r1, ={previous}",
        "ldr r1, [r1]",
        "str r0, [r1]",

        // 2. Previous thread contex has been saved. We can call the kernel_task now.
        // Calling convention was reverse-engineered.
        // Save frame pointer and LR for function call.
        "push {{r7, LR}}",
        // Move frame pointer for easier debugging.
        "mov r7, SP",
        // Call kernel_task.
        "bl {kernel_task}",
        // Restore frame pointer and LR.
        "pop {{r7, LR}}",

        // 3. Restore registers from next active thread.
        // First, we need to load its stack pointer, similar to the above process.
        "ldr r1, ={next}",
        "ldr r1, [r1]",
        "ldr r0, [r1]",

        // We can safely restore its registers, since it is either PSP (not affected by interrupts
        // during this PendSV handling, or we already made space for it in a previous switch from
        // the thread.
        "ldmia r0!, {{r4-r11, LR}}",

        // Depending on whether we are are returning to a thread on MSP or PSP, we need to set MSP
        // back to its original location, before it was moved to make space.
        "tst LR, #(1<<2)",
        "ite eq",
        // If we came from MSP, we need to load the original SP. It is stored in r0 again.
        "msreq MSP, r0",
        // Otherwise, restore PSP.
        "msrne PSP, r0",
        // ISB again after write to SP.
        "isb",

        // 4. Return to thread.
        "bx LR",
        previous = sym scheduler::PREVIOUS_TASK,
        next = sym scheduler::NEXT_TASK,
        kernel_task = sym kernel_task,
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

/// General kernel task to run scheduler, copy data, etc.
/// Requires C calling convention to be called from PendSV.
extern "C" fn kernel_task() {
    scheduler::schedule_next();
}