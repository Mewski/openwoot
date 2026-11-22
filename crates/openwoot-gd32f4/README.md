# openwoot-gd32f4

Chip HAL for the GigaDevice GD32F425 (Wooting 80HE) on `gd32f425-pac`: clocks, GPIO, interrupts with type-level binding, the embassy-time driver and executors, the backup-register mailbox of the factory bootloader, and the USB high-speed controller (`usb`).

## USB

`usb::Driver` binds `embassy-usb-synopsys-otg` 0.4.0 to the USBHS core (Synopsys DWC2, core id `0x1000`) with the external ULPI transceiver: ULPI pins on alternate function 10, the transceiver held in reset while pins and clocks are set up, 6 endpoints, 4096 bytes of FIFO RAM, interrupt `USBHS` bound to `usb::InterruptHandler`. `usb::SpeedLimit::Full` runs the high-speed transceiver at full speed. The core is initialised in the order of the C build (ULPI selection, core reset, turnaround time 9, timeout calibration 7, B-session valid override, VBUS sensing off, transceiver delay on, receive FIFO of 283 words plus the OUT endpoints). `usb::negotiated_speed` reports the speed of the last bus reset.

## Licence

Distributed under the licence of this repository. `embassy-usb-synopsys-otg` is a dependency under MIT OR Apache-2.0.
