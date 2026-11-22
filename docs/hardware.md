# Hardware

This page covers the main board of each supported keyboard as the Rust firmware uses it: the microcontroller, clocks, memory, every peripheral and pin the firmware touches, the interrupts and the DMA channels. The chip HALs (`crates/openwoot-gd32f4/`, `crates/openwoot-gd32f3/`) and the board crates (`boards/<board>/`) are authoritative; this page describes what they do. Timing figures are computed from the register settings, not measured.

| Board | Chip HAL | State |
| --- | --- | --- |
| Wooting 80HE | `openwoot-gd32f4` | Runs on the owner's keyboard: scanning, USB at high speed, LEDs and light bar, profiles, storage, diagnostics, update mode. |
| Wooting 60HE+ | `openwoot-gd32f3` | Builds and is host-tested; it has never run on a keyboard. See [Unverified on the Wooting 60HE+](#unverified-on-the-wooting-60he). |

Peripheral names are GigaDevice's, which number instances from 0 (`TIMER1` is STM32 TIM2, `DMA1` channel n on the GD32F425 is STM32 DMA2 stream n). The NOR layout and the device-information record are in [storage](storage.md); the factory bootloader, update mode and recovery are in [flashing](flashing.md).

## Where the facts live

| What | Where |
| --- | --- |
| Clock tree, USB ids, flash areas, the peripherals by board function (`Board::new`) | `boards/<board>/src/lib.rs` |
| Multiplexer lines, converter inputs, sample time, sensor maps | `boards/<board>/src/scan.rs` (`WIRING`) |
| LED driver address and set-up, LED tables, light bar | `boards/<board>/src/lighting.rs` |
| NOR chip select, NOR layout, device-information source | `boards/<board>/src/storage.rs` |
| Image area and RAM | `boards/<board>/memory.x` |
| Register set-up of every peripheral | the chip HAL: `rcu`, `gpio`, `scan`, `time`, `i2c`, `spi`, `usb`, `system`, `flash`, private `dma` and `basepri` |
| LED driver and NOR flash commands | `openwoot-drivers` (`is31fl3741`, `spi_nor`) |

## Microcontroller

| | Wooting 80HE | Wooting 60HE+ |
| --- | --- | --- |
| Chip | GigaDevice GD32F425 (pin-compatible with the GD32F427; the exact type and package are not determined) | GigaDevice GD32F303, extra-density device (package not determined) |
| Core | Arm Cortex-M4F, single-precision FPU, target `thumbv7em-none-eabihf` | the same |
| Flash | at least 1024 KiB at `0x0800_0000` | at least 1024 KiB at `0x0800_0000` |
| RAM | 192 KiB at `0x2000_0000` | 96 KiB at `0x2000_0000` |
| Vector table | 16 core + 91 device entries | 16 core + 60 device entries |
| Factory unique id (96 bit, `hal::flash::unique_id`, reported to the host) | `0x1fff_7a10` | `0x1fff_f7e8` |

The PACs are generated from GigaDevice's SVD files (`crates/gd32f425-pac/`, `crates/gd32f303-pac/`, with the patches listed in their READMEs).

## Clock tree

`rcu::ClockTree::configure` (called first in `Board::new`) returns to the internal oscillator, starts the PLL source, sets the regulator to its highest output level, enables the high-drive mode of the power unit and switches the system clock to the PLL. Flash wait states are never written. The board constant is `CLOCK_TREE`.

| | Wooting 80HE | Wooting 60HE+ |
| --- | --- | --- |
| Source | 25 MHz crystal (HXTAL); if it never becomes stable the firmware stops in a loop | internal 8 MHz oscillator (IRC8M) / 2 = 4 MHz; the crystal oscillator is never switched on |
| PLL | / 25 = 1 MHz, x 400 = 400 MHz VCO, / 2 | x 30 |
| System clock, AHB | 200 MHz, 200 MHz | 120 MHz, 120 MHz |
| APB1, APB2 | 50 MHz, 100 MHz | 60 MHz, 120 MHz |
| Timer clocks | 200 MHz on both buses (timer clock selection x 4) | 120 MHz on both buses |
| ADC clock | AHB / 5 = 40 MHz | APB2 / 4 = 30 MHz |
| SPI clock | APB2 / 2 = 50 MHz | APB1 / 2 = 30 MHz |
| I2C | 400 kHz fast mode from APB1 | 400 kHz fast mode from APB1 |
| USB clock | 60 MHz from the external ULPI transceiver (the PLL's 44.4 MHz output is unused) | system clock / 2.5 = 48 MHz |

`hal::time` and the scan trigger divide their timer clock down to 1 MHz (prescaler 199 on the 80HE, 119 on the 60HE+).

## Memory map

`memory.x` is the single source of the image area and RAM; `build.rs` turns it into the constant `IMAGE`, and each board tests that the image lies between the bootloader and the serial page. Nothing in the firmware erases or programs the microcontroller flash.

| Area | Wooting 80HE | Wooting 60HE+ |
| --- | --- | --- |
| factory bootloader, never written ([flashing](flashing.md)) | `0x0800_0000`-`0x0800_7fff` (32 KiB) | `0x0800_0000`-`0x0800_6fff` (28 KiB) |
| image (`FLASH` in `memory.x`): vector table, image descriptor, code | `0x0800_8000`-`0x080f_dfff` (984 KiB) | `0x0800_7000`-`0x080f_cfff` (984 KiB) |
| serial number page, read for the USB serial string, never written | `0x080f_e000`-`0x080f_efff` | `0x080f_d000`-`0x080f_dfff` |
| last page | `0x080f_f000`-`0x080f_ffff`, the bootloader's completion marker, never touched | `0x080f_e000`-`0x080f_efff` unused; `0x080f_f000`-`0x080f_ffff` factory device information, read in place ([storage](storage.md)) |
| RAM (`RAM` in `memory.x`) | `0x2000_0000`-`0x2002_ffff` | `0x2000_0000`-`0x2001_7fff` |

The image descriptor sits on the first 64-byte boundary behind the vector table ([architecture](architecture.md#image-descriptor)). Statics fill RAM from the bottom; the stack grows down from the top. `cortex-m-rt` paints the stack at reset (`paint-stack`), and `hal::diagnostics::hardware` reports its size and the deepest use since reset (`openwoot status`, `stack peak`). `cargo xtask check` prints the RAM budget per board (statics plus the stack bound of the call graph, with 2048 bytes to spare).

## Peripherals and owners

Each peripheral belongs to one type of the chip HAL, which takes the PAC peripheral by value; the board hands them over in `Board::new`.

| Function | Wooting 80HE | Wooting 60HE+ | Owner |
| --- | --- | --- | --- |
| scan trigger | `TIMER1` channel 1 | `TIMER1` channel 1 | `scan::Converter` (`scan/trigger`) |
| sensor converter | `ADC0` (with the common ADC registers) | `ADC0`, plus `ADC1` for the reference | `scan::Converter` (`scan/adc`) |
| scan transport | `DMA1` channel 0 | `DMA0` channel 0 | `scan::Converter` (`scan/transport`) |
| time driver | `TIMER4`, 32-bit counter | `TIMER4`, 16-bit counter | `time::start` |
| LED bus | `I2C0`, transmit on `DMA0` channel 6 | `I2C0`, transmit on `DMA0` channel 5 | `i2c::I2c` |
| NOR flash bus | `SPI0`, receive `DMA1` channel 2, transmit `DMA1` channel 3 | `SPI1`, receive `DMA0` channel 3, transmit `DMA0` channel 4 | `spi::Spi`, `spi::Device` |
| USB | USBHS with an external ULPI transceiver | USBD (full-speed device controller) | `usb::Driver` |
| key executor | UART4 interrupt vector (the UART itself is unused) | the same | `executor::run` |
| bootloader mailbox | RTC backup registers 0 to 2, PMU | BKP data registers 0 to 2, PMU | `system::Bootloader` |
| flash reads | memory-mapped flash | memory-mapped flash, the user option byte (`FMC_OBSTAT`) | `flash` |
| clocks | RCU, PMU | RCU, PMU | `rcu::ClockTree` |

DMA channels are reached only through the HAL's private `dma::Channel`, one type per controller and channel; each owner writes only its own channel registers and its own bits of the shared flag-clear register.

### Sensor scan

Each key has a hall-effect sensor on a 16:1 analog multiplexer. All multiplexers share four address lines; each has an enable line (low = on) and its own converter input. `TIMER1` channel 1 counts microseconds in single-pulse mode and its compare edge triggers each conversion; the DMA channel moves each 32-bit result word into the sample buffer (circular, 1 word in single-input mode, one per input in parallel mode); the end-of-conversion interrupt runs the scanner. Address lines are written with one store of a precomputed set/reset word; enables and address lines are push-pull outputs at 50 MHz.

| | Wooting 80HE | Wooting 60HE+ |
| --- | --- | --- |
| multiplexers, addresses used | 6, addresses 0 to 14 | 4, addresses 0 to 15 |
| address lines (bit 0 to 3) | PA8, PA9, PA10, PA11 | PB0, PB1, PB10, PB11 |
| enable lines, multiplexer 0 to n | PB14, PB15, PC6, PC7, PC8, PC9 | PB8, PB9, PC13, PC14 |
| converter inputs (pin: channel) | PA0: 0, PA1: 1, PA2: 2, PC1: 11, PC5: 15, PC4: 14 | PA0: 0, PA1: 1, PA2: 2, PA3: 3 |
| reference input | none; fixed zero-field level 2048 | PA4, channel 4 on `ADC1`, converted with every sample (regular parallel mode); level 2482 while it reads 0 |
| sample time, oversampling | 3 cycles, none: 0.375 µs per conversion | 1.5 cycles, 8 x: about 3.7 µs per result |
| result word | low 16 bits the sensor | low 16 bits the sensor, high 16 bits the reference |
| second conversion | inputs 3 to 5 (`RecheckFrom<3>`) | none |
| converter interrupt | `ADC` | `ADC0_1` |
| overruns | `ADC0` overrun flag counted (`scan overruns` in `openwoot status`); the transport restarts | not detected |
| layout variant | from the device-information record | bit 0 of the user option byte |

Between cycles the trigger source is parked (80HE: `TIMER0` channel 0, which never runs; 60HE+: software trigger). A cycle that does not end within 50 ms is restarted by the key task (`scan::Converter::recover`).

### Time driver

`hal::time` is the embassy-time driver at 1 MHz (`TICK_HZ`) on `TIMER4`. The update interrupt and a compare at half the counter range (channel 0) extend the count to 64 bits; channel 1 is the alarm. The 80HE counter is 32 bits wide, the 60HE+ counter 16 bits. The scan reads the low 16 bits of the counter directly.

### LED bus and LED driver

Both boards drive their LEDs with one ISSI IS31FL3741 (9 switch lines x 39 current sinks) at 7-bit address `0x30`, through `openwoot_drivers::is31fl3741`. `hal::i2c::I2c` only writes: start, address, then the DMA channel sends each chunk; the event, error and DMA interrupts wake the waiting task. A transfer that does not finish within 10 ms fails with `Error::Timeout` and the bus is reset; results are counted (`led bus transfers`, `led bus errors`, `led bus last error` in `openwoot status`).

| | Wooting 80HE | Wooting 60HE+ |
| --- | --- | --- |
| clock, data | PB6, PB7 (alternate function 4, open drain) | PB6, PB7 (alternate function, open drain) |
| DMA | `DMA0` channel 6, sub-peripheral 1, interrupt `DMA0_Channel6` | `DMA0` channel 5, interrupt `DMA0_Channel5` |
| enable line | PB8, high after the driver's software reset | PB5, the same |
| running configuration | `0x11`: switch lines SW1 to SW8 | `0x31`: SW1 to SW6 |
| LED numbers | 1 to 104 (105 = none); 10 light-bar LEDs (51, 64, 77, 90, 103, 52, 65, 78, 91, 104) | 0 to 107 (108 = none); no light bar; red limited to 232 |
| frame | 10 pages of up to 36 registers | 7 pages of up to 30 registers |

Start-up: software reset, shutdown, enable line high, all scaling registers `0xff`, global current `0xff`, pull-up/pull-down register `0x35`; the running configuration is written when lighting starts.

### NOR flash bus

`hal::spi::Spi` is a mode-0 master, 8 bit, most significant bit first, with software chip select; every transfer runs by DMA in both directions and the receive channel's interrupt wakes the waiting task. Details and the chip layout: [storage](storage.md).

| | Wooting 80HE | Wooting 60HE+ |
| --- | --- | --- |
| clock, data out, data in | PB3, PA7, PB4 (alternate function 5) | PB13, PB15 (alternate function), PB14 (floating input) |
| chip select | PD2 | PB12 |
| DMA | receive `DMA1` channel 2, transmit `DMA1` channel 3 (sub-peripheral 3), interrupt `DMA1_Channel2` | receive `DMA0` channel 3, transmit `DMA0` channel 4, interrupt `DMA0_Channel3` |

### USB

| | Wooting 80HE | Wooting 60HE+ |
| --- | --- | --- |
| controller | USBHS (Synopsys DWC2), through `embassy-usb-synopsys-otg` | USBD, a port of embassy-stm32's fsdev driver |
| PHY | external ULPI transceiver, 60 MHz clock from the transceiver | internal full-speed transceiver; the controller has no D+ pull-up |
| pins | ULPI (alternate function 10, maximum speed): clock PA5, direction PC2, next PC3, stop PC0, data 0 to 7 PA3, PB0, PB1, PB10, PB11, PB12, PB13, PB5; PC10 low while the pins and clocks are set up, then high (presumably the transceiver's reset) | D- PA11, D+ PA12 (fixed); D+ pull-up switched by PA15, freed from JTAG (serial-wire debug stays) |
| speed | high speed, or full speed when the stored setting forces it (`usb::SpeedLimit::Full`) | full speed only; the high-speed-only descriptor requests are stalled |
| resources | 6 endpoints, 4096 bytes of FIFO RAM, receive FIFO of 283 words plus the OUT endpoints | 8 endpoint registers, 512 bytes of packet memory (64-byte descriptor table, 128 bytes for endpoint 0); no isochronous endpoints |
| interrupts | `USBHS` | `USBD_HP_CAN0_TX`, `USBD_LP_CAN0_RX0` |
| VBUS | not sensed; B-session valid overridden | not sensed |

`hal::usb::negotiated_speed` reports the speed of the last bus reset (`usb speed` in `openwoot status`).

### Bootloader mailbox

Three backup registers survive a reset and carry requests between the firmware and the factory bootloader: register 0 = `0x504` keeps the bootloader in update mode for one start (`UpdateMode::enter_update_mode`), register 1 = `0x420` is the bootloader's format request, which `Bootloader::take_format_request` clears, and register 2 is the option byte of a restart. Writes open the backup domain through the PMU; on the 60HE+ every access also switches on the backup interface clock and sets the tamper-pin reset bit. See [flashing](flashing.md).

## Interrupts and critical sections

| Priority | Interrupts |
| --- | --- |
| 0 | the scan's end-of-conversion interrupt; never masked |
| 1 | the time driver (`TIMER4`), the LED bus (`I2C0` event and error, its DMA channel), the NOR bus (receive DMA channel) |
| 2 | USB |
| 3 | the key executor (UART4 vector), which runs the board's `scanning` task |
| thread mode | the thread executor: USB device, configuration channel, reports, lighting, profiles |

Critical sections (`critical-section` with the HAL's `basepri` implementation) raise BASEPRI to `0x10`: they mask levels 1 to 15 and never the scan, which takes no critical section. `Priority` cannot express level 0 outside the HAL. Fault vectors and every unclaimed vector end in the `cortex-m-rt` default handler; there is no watchdog.

## DMA priorities

| Channel | Wooting 80HE | Wooting 60HE+ | Priority |
| --- | --- | --- | --- |
| scan transport | `DMA1` channel 0 | `DMA0` channel 0 | ultra high |
| LED bus transmit | `DMA0` channel 6 | `DMA0` channel 5 | ultra high |
| NOR bus receive and transmit | `DMA1` channels 2 and 3 | `DMA0` channels 3 and 4 | low |

The scan channel shares its controller with the NOR bus on both boards. The converter holds one result at a time and a new conversion overwrites it; if the SPI channels win arbitration while a flash transfer runs, the scan's transfer can come too late. On the GD32F425 the converter then flags an overrun and stops requesting DMA, and the scan stalls. The scan channel therefore runs above the SPI channels on both boards.

## Lessons from the Wooting 80HE

- The GD32F425 USBHS accepts OUT data only on an enabled endpoint with a packet count, as GigaDevice's library programs it; `embassy-usb-synopsys-otg` clears NAK only after the first transfer. The HAL therefore arms endpoint 0 before every control OUT data stage and before the status stage that follows the last IN data packet, and re-arms each OUT endpoint after every packet read (`usb::ControlPipe`, `usb::OutEndpoint`).
- The bus can report suspend before the first bus reset. A bus reset ends the suspended state; without that, the device stays "suspended" after enumeration and sends no reports.
- The scan DMA channel must have priority over the SPI channels (see [DMA priorities](#dma-priorities)); with equal priority a settings write stalled the scan. The HAL counts converter overruns and restarts the transport, and the key task restarts a cycle that does not end within 50 ms.

## Unverified on the Wooting 60HE+

Every 60HE+ fact on this page comes from that board's stock firmware and has not been confirmed on hardware. In particular:

- the exact chip and package, and whether the board carries a crystal the firmware does not use;
- every pin, the function of PB5 (presumably the LED driver's shutdown input) and the wiring of the D+ pull-up on PA15;
- the reference source on PA4 and its zero-field level;
- the USBD port and its control-transfer handling (host-tested against a model of the endpoint register only);
- the factory bootloader: how its update mode is entered and left and what it does with the backup registers; the image descriptor names no update mode, so `openwoot flash` refuses the board ([flashing](flashing.md)).

On both boards the types of the hall sensors, the multiplexers and the NOR chip, the 80HE's USB transceiver and the function of PC10 and PB8 are not determined, and the scan rate, the I2C transfer time and the interrupt latency were computed, not measured.
