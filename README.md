# Preemptive Context Switching on STM32

This project presents a basic implementation of context switching as it might be implemented by an RTOS.
is intended to be run on a STM32F401RE-based board.

## Preparation

You need Rust and the `arm-none-eabi-gcc` toolchain installed on your system.
Run `rustup target add thumbv7em-none-eabi` to install the required Rust target.

## Building

To build it run:

```
cargo build
arm-none-eabi-objcopy -O binary .\target\thumbv7em-none-eabi\debug\stm32-context-switch-example stm32-image.bin
```

If your STM32 board presents itself as a mass storage device, you should be able to just copy
`stm32-image.bin` onto it to flash it.

## Running

You should be able to view the output sent by the program via UART e.g. with gnu screen.

## Acknowledgements

Much of the theory and specific assembly code behind this project is based on the following articles/repositories:

- https://www.adamh.cz/blog/2016/07/context-switch-on-the-arm-cortex-m0/
- https://github.com/adamheinrich/os.h/
- https://blog.stratifylabs.dev/device/2013-10-09-Context-Switching-on-the-Cortex-M3/