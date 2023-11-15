use crate::task::{OS_CURRENT_TASK, OS_NEXT_TASK};

#[naked]
#[no_mangle]
#[allow(non_snake_case)]
fn PendSV() {
    unsafe {
        core::arch::asm!(
        // 1. Save r4-r11
        "push {{r4-r11, r14}}",

        // 2. Save stack pointer to task control block
        "ldr r0, ={0}", // Load address of OS_CURRENT_TASK into r0
        "ldr r0, [r0]", // Load contents of OS_CURRENT_TASK into r0
        "str SP, [r0]", // Store stack pointer into TSB

        // 3. Load next stack pointer from next TSB
        "ldr r0, ={1}", // Load address of OS_NEXT_TASK into r0
        "ldr r0, [r0]", // Load contents of OS_NEXT_TASK into r0
        "ldr SP, [r0]", // Store stack pointer into TSB

        // 4. Restore r4-r11
        "pop {{r4-r11, r14}}",

        // 5. Return in thread mode
        // "ldr r14, =0xFFFFFFFD",
        "bx r14",
        sym OS_CURRENT_TASK,
        sym OS_NEXT_TASK,
        options(noreturn),
        )
    };
}
