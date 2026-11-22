# openwoot-gd32f4

Chip HAL for the GigaDevice GD32F425 (Wooting 80HE) on `gd32f425-pac`: clocks (`rcu::ClockTree`), GPIO, interrupts with type-level binding, the embassy-time driver and executors, the I2C master of the LED bus and the SPI master of the NOR flash (both by DMA), the hall-sensor converter (`scan`), memory-mapped flash reads, the hardware diagnostics, the backup-register mailbox of the factory bootloader, and the USB high-speed controller (`usb`).

## USB

`usb::Driver` binds `embassy-usb-synopsys-otg` 0.4.0 to the USBHS core (Synopsys DWC2, core id `0x1000`) with the external ULPI transceiver: ULPI pins on alternate function 10, the transceiver held in reset while pins and clocks are set up, 6 endpoints, 4096 bytes of FIFO RAM, interrupt `USBHS` bound to `usb::InterruptHandler`. `usb::SpeedLimit::Full` runs the high-speed transceiver at full speed. The core is initialised in the order of the C build (ULPI selection, core reset, turnaround time 9, timeout calibration 7, B-session valid override, VBUS sensing off, transceiver delay on, receive FIFO of 283 words plus the OUT endpoints). `usb::negotiated_speed` reports the speed of the last bus reset.

The GD32F425 USBHS accepts OUT data only on an enabled endpoint with a packet count, as GigaDevice's library programs it (`usb_transc_outxfer`: `PCNT = 1`, `TLEN` = max packet size, `EPEN | CNAK`); `embassy-usb-synopsys-otg` 0.4.0 only clears NAK after the first transfer. The glue therefore wraps the control pipe and the OUT endpoints: it arms endpoint 0 for every control OUT data stage and for the status stage after the last IN data packet, and re-arms each OUT endpoint after every packet read.

## Licence

Distributed under the licence of this repository. `embassy-usb-synopsys-otg` is a dependency under MIT OR Apache-2.0.
