# openwoot-gd32f3

Chip HAL for the GigaDevice GD32F303 (Wooting 60HE+) on `gd32f303-pac`: clocks, GPIO, interrupts with type-level binding, the embassy-time driver and executors, the backup-register mailbox of the factory bootloader, and the USB full-speed device controller (`usb`).

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
