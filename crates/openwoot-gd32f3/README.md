# openwoot-gd32f3

Chip HAL for the GigaDevice GD32F303 (Wooting 60HE+) on `gd32f303-pac`: clocks (`rcu::ClockTree`), GPIO, interrupts with type-level binding, the embassy-time driver and executors, the I2C master of the LED bus and the SPI master of the NOR flash (both by DMA), the hall-sensor converter (`scan`, with the reference converter and oversampling), memory-mapped flash reads and the user option byte, the hardware diagnostics, the backup-register mailbox of the factory bootloader, and the USB full-speed device controller (`usb`).

## Executors and interrupts

`executor::run(init)` first settles what the factory bootloader left behind (stops SysTick, disables and unpends every device interrupt, clears BASEPRI and PRIMASK) and selects priority group 0 (all four priority bits pre-empt), then starts the key executor on the UART4 vector at `Priority::KEYS` (3), the device executor on UART3 at `Priority::DEVICE` (4) and the lighting executor on USART2 at `Priority::LIGHTING` (5), the UARTs themselves unused, and hands `init` the thread executor's `Spawner` and the three `SendSpawner`s (`executor::Spawners { thread, keys, device, lighting }`). Above the executors run USB (`Priority::USB`, 2), the time driver and the I2C and SPI interrupts (`Priority::HIGHEST`, 1) and the scan interrupt at 0, which `Priority` cannot express outside the HAL; the BASEPRI critical section masks levels 1 to 15 and never the scan. `bind_interrupts!` defines the vectors and a struct that implements `interrupt::Binding` for each; a reference to a binding is one too, so a board can take its bindings as a trait object and stay non-generic.

`i2c::I2c` sends a chunk whose data lies below the SRAM (a constant in the image) through a 16-byte staging buffer, as the GD32F425 HAL does, so its DMA channel never reads the flash.

## USB

`usb::Driver` implements `embassy_usb_driver::Driver` for the USBD controller (the STM32 "fsdev" design): 8 endpoint registers with toggle-on-write status bits and 512 bytes of packet memory, of which the descriptor table takes 64 bytes and endpoint 0 a fixed 128 bytes. The controller has no D+ pull-up; the driver switches the board's pull-up pin (the 60HE+ uses PA15, freed from JTAG by the driver). Both controller interrupts (`USBD_HP_CAN0_TX`, `USBD_LP_CAN0_RX0`) are bound to `usb::InterruptHandler`; the wake-up interrupt is not used. Isochronous endpoints are not supported.

The endpoint-register model, the packet-memory layout and the control-transfer state changes are pure functions with host tests against a model of the register's write semantics.

## Licence and attribution

The USB driver's structure (endpoint and packet-memory allocation, the bus, endpoint and control-pipe state machines) is ported from `embassy-stm32` 0.6.0, `src/usb/usb.rs`, of the [Embassy project](https://github.com/embassy-rs/embassy), licensed under MIT OR Apache-2.0; this crate uses it under the MIT licence:

> Copyright (c) Embassy project contributors
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

The rest of the crate is distributed under the licence of this repository.
