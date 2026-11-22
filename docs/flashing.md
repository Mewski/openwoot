# Flashing

**This guide is about the Wooting 80HE.** It describes that board's factory bootloader and the update procedure confirmed on that board. For the Wooting 60HE+ none of it is established: how its factory bootloader decides whether to start an application, how its update mode is entered and left, and how a keyboard is brought back after a bad image are not known. `tools/openwoot.py` refuses `enter` and `flash` for the 60HE+ before it sends anything (its `boards/wooting_60he_plus/board.json` marks the update procedure as unknown), and `make BOARD=wooting_60he_plus flash` stops at the same refusal. Do not flash the 60HE+ build by any other means.

**The restructured build of this tree has not yet been run on hardware.** Everything this guide reports as confirmed on a Wooting 80HE was done with an earlier build of openwoot for that board. The update procedure itself does not depend on the build.

This document is the practical guide to installing firmware on a Wooting 80HE: what you need, the commands of a normal update and what they print, what the keyboard shows meanwhile, how to check the result, how to return to the stock firmware, and how to get the keyboard back when an image does not work. The last sections say what an update does not touch and which risks remain.

Related documents: [building](building.md) (how the image is built), [`tools/README.md`](../tools/README.md) (installation and every sub-command of the host tool `tools/openwoot.py`), [bootloader](bootloader.md) (the reference for the factory bootloader and its update protocol; this guide only uses it), [storage](storage.md) (the settings in the SPI flash), [protocol](protocol.md) (the host protocol of the running firmware).

Terms: the *firmware* is the program that makes the keyboard a keyboard, openwoot or the stock firmware. The *factory bootloader* is a separate program in the first 32 KiB of the microcontroller's flash. It runs first after every reset and either starts the firmware or stays in *update mode*, in which it appears on USB as "Wooting Restore (80HE)", accepts new firmware and does not type. The *completion marker* is one word of flash with which the bootloader remembers whether the last update was finished.

## State of testing

Everything in this guide was tried on one keyboard: a Wooting 80HE with the ANSI layout and bootloader version 2.3, from a macOS host, with an earlier build of openwoot.

| Subject | State |
| --- | --- |
| The update procedure of this guide (`enter`, `flash`) | Confirmed on a Wooting 80HE: three complete updates, each verified by the CRC, the firmware back on USB about one second after the final command. |
| An earlier build of openwoot for the 80HE with `LTO=0` (the default) | Confirmed on a Wooting 80HE: USB enumeration, host queries, lighting, typing, Fn combinations, profile switching, rapid trigger. |
| The restructured build of this tree (`make BOARD=wooting_80he`), with either `LTO` setting | Not yet tested on hardware. |
| Settings and profiles across an update | Confirmed on a Wooting 80HE: the firmware reported the same profiles and settings before and after each update. |
| The recovery key combination | Confirmed on a Wooting 80HE while working firmware was installed, including the way back out of update mode without writing anything. With firmware that does not start: not yet tested on hardware. |
| An earlier build with `LTO=1` | Not yet tested on hardware. |
| Gamepad and XInput modes, suspend and resume, long-term stability | Not yet tested on hardware. |
| The vendor's configuration program (Wootility) against openwoot | Not yet tested on hardware. |
| ISO and JIS boards | Not yet tested on hardware. |
| Writing Wooting's own update file with `tools/openwoot.py` | Not yet tested on hardware. The procedure is the one confirmed above. |
| An interrupted update and the indication for missing firmware | Not yet tested on hardware. |
| The host tool on Linux and Windows | Not yet tested, see [`tools/README.md`](../tools/README.md). |

## What you need

- **The keyboard**, connected with its cable. Connect one Wooting keyboard at a time; the tool uses the first one it finds.
- **A second keyboard.** In update mode the 80HE does not type, and `flash` asks you to type a word before it writes. Without one, use `make BOARD=wooting_80he flash` or the command line shown in [`tools/README.md`](../tools/README.md); both skip the question.
- **Wootility closed**, including a browser tab with its web version. Two programs that talk to the same HID interface read each other's replies. Wootility also offers to restore a keyboard it finds in update mode; see [Wootility](#wootility).
- **A built image**: `build/wooting_80he/openwoot-wooting_80he.hex`, built with `make BOARD=wooting_80he`, see [building](building.md).
- **The host tool set up**: Python 3 with the package `hidapi`, and on Linux a udev rule; see [`tools/README.md`](../tools/README.md).
- **The key combination in your fingers.** It is the way back from firmware that does not start. Practise it once with `python3 tools/openwoot.py combo` before you install anything you changed; see [Recovery with the key combination](#recovery-with-the-key-combination).

Before the first image of your own, install the unmodified sources built with the default settings. If that image works on your keyboard and your changed image does not, the fault is in the new code. Keep in mind that the unmodified build of this tree is not yet tested on hardware either: until it has been, be ready to use the [key combination](#recovery-with-the-key-combination) and to return to the stock firmware even with the unmodified sources.

## The normal update

All commands are run in the root of the repository.

### 1. Build

```sh
make BOARD=wooting_80he
```

### 2. Look at the image, offline

```sh
python3 tools/openwoot.py plan build/wooting_80he/openwoot-wooting_80he.hex
```

```
board         : Wooting 80HE (boards/wooting_80he)
image         : build/wooting_80he/openwoot-wooting_80he.hex, 495872 bytes after padding
blocks        : 1937 of 256 bytes
crc           : 0xdcbea4
```

This step sends nothing. The tool parses the file, finds the board whose start address and initial stack pointer the image has (`boards/wooting_80he/board.json`), and refuses it unless it starts at `0x08008000`, its first word is the stack pointer `0x20030000`, its reset vector points into the image and it fits into the 3936 blocks the bootloader can write. It prints the number of 256-byte blocks and the CRC that the bootloader must report after the transfer. The numbers above are those of the unmodified sources of this tree built with GCC 10.3.1 and `LTO=0`; they change with every change to the code.

### 3. Enter update mode

```sh
python3 tools/openwoot.py enter
```

```
firmware version 2.14.1
asking the firmware to reset into update mode (feature command 2, argument 0)
update mode: 31e3:140f "Wooting Restore (80HE)"
```

The running firmware, stock or openwoot, resets the keyboard into the bootloader. Update mode appeared 2 to 9 seconds after the command in the confirmed runs; the tool waits up to 20 seconds. Nothing in flash has changed at this point. To leave update mode without an update, run `python3 tools/openwoot.py reboot`, or unplug the keyboard and plug it in again.

openwoot reports the firmware version 2.14.1, the version of the stock firmware it is compatible with.

### 4. Write the image

```sh
python3 tools/openwoot.py flash build/wooting_80he/openwoot-wooting_80he.hex
```

The output of the confirmed runs had this form; `<size>`, `<blocks>` and `<crc>` are the values that `plan` prints for the image:

```
board         : Wooting 80HE (boards/wooting_80he)
image         : build/wooting_80he/openwoot-wooting_80he.hex, <size> bytes after padding
blocks        : <blocks> of 256 bytes
crc           : <crc>
update mode   : 31e3:140f "Wooting Restore (80HE)"
flags         : 0x02 (update mode requested by the firmware)
bootloader    : version 2.3
serial        : <serial number>
session crc   : 0x000000 (blocks written since the bootloader started)
type ERASE to invalidate the installed firmware and write <blocks> blocks: ERASE
command 6: unlock the write commands
command 2: start upload of <blocks> blocks
  block 1 / <blocks>
  block 101 / <blocks>
  ...
  block <blocks> / <blocks>
all blocks written in 24.4 s
CRC verified  : <crc>
command 5 with bit 0: write the completion marker and start the firmware
firmware is back: 31e3:1402 "Wooting 80HE"
```

What happens, in the words of the output:

1. The checks of `plan` are repeated and the bootloader is asked for its state. Up to the question nothing has been written.
2. You type the word `ERASE` on the second keyboard. Any other answer ends the command with nothing changed. The option `--yes` skips the question.
3. Command 2 erases the completion marker. **From this moment the installed firmware counts as invalid**: until an update is completed, the bootloader stays in update mode at every start.
4. The blocks are written in ascending order. The bootloader erases each 4 KiB page of flash when the first block of that page arrives.
5. The tool asks the bootloader for the index of the block written last and for the CRC over what was written, and compares both with the image. The bootloader itself verifies nothing, so this comparison is the only check of the transfer.
6. Only after the CRC matched, the tool sends the final command. The bootloader writes the completion marker and resets, and the new firmware starts.

The transfer of about 1930 blocks takes about 25 seconds, and the firmware is back on USB about one second after the final command (confirmed on a Wooting 80HE). The tool waits up to 20 seconds for it.

`make BOARD=wooting_80he flash` runs steps 1, 3 and 4 in one go and passes `--yes` to `flash`: it does not ask the question, and the installed firmware is invalidated as soon as the keyboard is in update mode. [Bootloader](bootloader.md) describes every command of the transfer.

### What an update writes

The microcontroller has 1 MiB of internal flash. An update writes one part of it:

| Range | Content | Touched by an update |
| --- | --- | --- |
| `0x08000000`-`0x08007FFF` | The factory bootloader | Never. No update command can address it. |
| `0x08008000`-`0x080FDFFF` | The firmware | Yes: block `n` of the image goes to `0x08008000 + 256 * n`. Only the 4 KiB pages that receive a block are erased; what a longer, older image left behind the end of the new one stays there and is never executed. |
| `0x080FE000`-`0x080FEFFF` | A copy of the serial number, written by the firmware | No. |
| `0x080FF000`-`0x080FFFFF` | The completion marker, in the last word at `0x080FFFFC` | Erased when the transfer starts, written when it is finished. |

[Bootloader](bootloader.md#flash-ranges-and-what-an-update-can-reach) is the reference for these ranges. The SPI flash with the settings is a separate chip that an update does not touch; see [The settings in the SPI flash](#the-settings-in-the-spi-flash).

## What the keyboard shows

In update mode all lighting of the firmware is off, no key types, and the bootloader lights a few keys to say why it is in update mode:

| Reason | Indication | On hardware |
| --- | --- | --- |
| The firmware asked for it (`enter`) | The Space bar lights yellow. | not yet tested on hardware |
| The key combination was held | The top-right key and the Right arrow key light blue. | confirmed on a Wooting 80HE |
| No valid firmware is installed: an update was begun and not completed | The E and R keys light red. | not yet tested on hardware |

During the transfer the light bar (the ten LEDs to the right of the Enter key) shows the progress: all ten dim red at the start, turning green one after the other (not yet tested on hardware). The bar follows the block numbers the host sends. It does not show that the flash content is correct; only the `CRC verified` line of the tool does.

After the final command the keyboard resets, and the firmware starts with its normal lighting. [Bootloader](bootloader.md#led-indications) has the exact positions and LED ids.

## Checking the result

`flash` ended with `CRC verified` and `firmware is back`: the application area of the flash holds exactly the bytes of the image file, and something that answers on the firmware's configuration interface is running.

Then query the firmware. `info` sends only commands that read:

```sh
python3 tools/openwoot.py info
```

```
device        : Wooting 80HE, product id 0x1402, serial <serial number>
firmware      : 2.14.1
usb speed     : high
mcu unique id : <24 hexadecimal digits>
spi flash     : manufacturer 0xef, device 0x16, unique id <16 hexadecimal digits>
device config : 0000000011000c (MCU read protection off)
active profile: index 0, namespace 0
profiles      : 4
lock mode     : 0
```

What to look at:

- Run `info` before the update as well and compare. The number of profiles, the active profile and the lock mode come from the settings storage. If they are unchanged, the new firmware has mounted the existing file system and read it.
- The output cannot tell openwoot from stock firmware 2.14.1. openwoot reports that version on purpose, and it answers these queries in the same way. What is installed is established by the CRC comparison of `flash`, not by `info`.
- The product name and the product id depend on the edition of the keyboard and on its USB mode.

After that, use the keyboard: type, look at the lighting, change brightness and effect with the Fn combinations, switch profiles. The first table of this document lists what has been confirmed this way and what has not.

## Returning to the stock firmware

Flash the update file from Wooting with the same tool. Wooting's update file for the 80HE is an Intel HEX text file for `0x08008000`, whatever its name; this is the format `tools/openwoot.py` reads.

```sh
python3 tools/openwoot.py plan STOCK_UPDATE_FILE
python3 tools/openwoot.py enter
python3 tools/openwoot.py flash STOCK_UPDATE_FILE
```

- STOCK_UPDATE_FILE stands for the path of that file. This repository does not contain it and names no download location, because none has been verified; obtain the file for your model from Wooting.
- For the update file of stock firmware 2.14.1, `plan` prints 1943 blocks and the CRC `0xbba5a6`.
- If the installed firmware does not answer `enter`, use the [key combination](#recovery-with-the-key-combination) instead.
- Profiles and settings stay as they are. The tool never asks the bootloader for a settings reset, and openwoot uses the storage formats of stock firmware 2.14.1 ([storage](storage.md)).

Writing Wooting's own file with this tool is not yet tested on hardware. The bootloader treats every image alike: it is given blocks and checks nothing about their content ([bootloader](bootloader.md)).

## Recovery with the key combination

The bootloader can be told to stay in update mode without any help from the installed firmware. This is the way back when an image was written completely and does not work: it does not appear on USB, crashes, hangs, or no longer answers `enter`.

1. Unplug the keyboard.
2. Press the **top-right key** and the **Right arrow key** all the way down and hold them.
3. Plug the keyboard in while holding them. **Both keys must be fully down before the plug goes in**: the bootloader samples the two keys once, within the first milliseconds after power-up, and does not look again.
4. The two keys light blue, and the keyboard appears on USB as "Wooting Restore (80HE)". Release the keys.
5. Flash a known good image:

   ```sh
   python3 tools/openwoot.py flash build/wooting_80he/openwoot-wooting_80he.hex
   ```

If the keyboard starts its firmware instead, the keys were not fully down in time. Unplug it and try again.

Confirmed on a Wooting 80HE with working firmware installed: with both keys pressed completely before plugging in, the bootloader stayed in update mode and lit the two keys blue; in an attempt with the keys not yet fully down, the firmware started. With firmware that does not start, the procedure is not yet tested on hardware. It does not depend on the firmware, because the bootloader reads the keys before it looks at the firmware at all ([bootloader](bootloader.md#the-key-combination)).

To practise without writing anything:

```sh
python3 tools/openwoot.py combo
```

The command waits for the keyboard to be unplugged and plugged in again, reports whether the firmware started (combination not detected) or the bootloader saw the keys, and in the second case leaves update mode again after a few seconds with a plain restart. It sends requests that read and that restart; it writes no flash.

By hand: in update mode, `python3 tools/openwoot.py bootinfo` prints the flags (`0x01` for the key combination), and `python3 tools/openwoot.py reboot` restarts into the installed firmware.

## An interrupted update

An update can stop anywhere: the cable is pulled, the host program is ended, the tool reports a status other than success, a wrong last block or a CRC mismatch. The tool then stops with a message that ends with `The keyboard stays in update mode; run the flash command again.` and does not send the final command.

What this leaves behind depends on whether the transfer had begun:

| The update stopped | State of the keyboard | How to continue |
| --- | --- | --- |
| before the line `command 2: start upload` | Nothing was changed. | `python3 tools/openwoot.py reboot`, or unplug and plug in: the installed firmware starts. |
| after it, at any point | The completion marker is erased and the firmware area holds part of the new image. The bootloader stays in update mode at every start, also after unplugging, and lights the E and R keys red. No key needs to be held. | Run the `flash` command again, with the same image or another one. The transfer starts again from block 0. |

If the bootloader stops answering after an interruption, unplug the keyboard, plug it in again and run `flash` again; it comes back in update mode by itself.

All of this follows from how the bootloader uses the completion marker ([bootloader](bootloader.md#recovery-scenarios)). No update has been interrupted on purpose: this section is not yet tested on hardware.

## What can happen

| Outcome | What you see | What to do |
| --- | --- | --- |
| The transfer fails or is interrupted. | The tool stops with a message; the final command is not sent. | [An interrupted update](#an-interrupted-update): run `flash` again. |
| The transfer completes and the firmware appears on USB. | `firmware is back: ...` | [Check the result](#checking-the-result). |
| The transfer completes and no firmware appears within 20 seconds. | The tool stops with `openwoot: the firmware did not appear within 20 s`. | The image was written correctly and marked complete, so the bootloader starts it at every power-up. Use the [key combination](#recovery-with-the-key-combination) and flash a known good image. |
| The firmware runs, answers on USB and misbehaves. | | `enter`, then flash another image. If `enter` fails, the key combination. |

The third row did not occur with the earlier `LTO=0` build that was tested on hardware. It is the case to prepare for with every other image: the build of this tree, which is not yet tested on hardware, the `LTO=1` build, and any source tree you changed.

## The settings in the SPI flash

Profiles and settings are not in the microcontroller. They are in a file system on a separate 8 MiB SPI flash chip, and the last sector of that chip holds a record with factory data: the serial number, the physical layout and the lock mode ([storage](storage.md)).

- **A firmware update does not touch them.** The bootloader never accesses the SPI flash, and the tool never sends the bootloader's settings-reset command. openwoot and stock firmware 2.14.1 use the same formats, so the settings are kept when one replaces the other. Confirmed on a Wooting 80HE for the change from the stock firmware to openwoot; the change back is not yet tested on hardware.
- **Firmware that mishandles them can damage them.** The firmware reads the chip at every start and writes it whenever a setting is stored. A fault in changed storage code can corrupt the file system, and the firmware formats a file system it cannot mount, without asking (`storage_init` in `src/storage/storage_init.c`). A fault can also overwrite the factory record, which no factory reset recreates.
- **No backup or restore tool exists.** Nothing in this repository saves the content of the SPI flash to a file or writes it back. Expect that profiles and settings can be lost when you run firmware you changed, and note the serial number that `info` prints before you start.

## Risks

**Firmware that programs the microcontroller's flash can destroy the bootloader.** No update command can reach the bootloader's flash, but firmware can: the flash controller erases and programs whatever address it is given. openwoot does this in exactly one place, as the stock firmware does: `storage_mirror_serial_to_mcu_flash` in `src/storage/storage_serial.c` has `hal_mcu_flash_write_page` (`hal/gd32f4xx/hal_mcu.c`) erase the 4 KiB page at `0x080FE000` (`BOARD_MCU_SERIAL_PAGE_ADDR`) and program the serial number into it. If you change firmware, leave that function and its address alone and add no other code that calls `hal_mcu_flash_write_page` or the flash controller functions of the library (`fmc_unlock`, `fmc_page_erase`, `fmc_halfword_program` and their relatives). Whether the bootloader's pages are write-protected by the option bytes of the microcontroller is not known; assume that nothing but correct addresses stands between firmware and the bootloader. A damaged bootloader cannot be repaired over USB by anything in this guide: every way back described here is the bootloader. It would take a hardware debug probe on the SWD pins of the microcontroller, and this project has not located a debug connector on the board and has never used one.

**The bootloader never verifies what it writes.** It answers every write command with success, reads nothing back and compares no checksum, neither during the update nor when it starts the firmware. The CRC comparison by the host tool is the only check. Never send the final command by hand after a transfer that was not verified: `python3 tools/openwoot.py reboot 1` sends exactly that command and marks whatever is in flash as complete firmware.

**Complete but faulty firmware is started every time.** The bootloader has no test for working firmware, no watchdog and no time-out. The key combination is the only way back, it is manual, and with firmware that does not start it is not yet tested on hardware. It also needs the bootloader, the two hall sensors and the ADC to work, and firmware can switch it off for one start: bit 1 of the option byte that the firmware's reset commands hand to the bootloader makes it ignore the keys once ([bootloader](bootloader.md#the-option-bits-in-rtc_bkp2)).

**Keep a way into update mode in firmware you change.** `enter` works only while the main loop runs and feature command 2 still reaches `hal_system_enter_update_mode` (handler `protocol_cmd_enter_update_mode` in `src/protocol/protocol_device.c`). An effect or key feature that blocks the main loop for good takes this way away, and the key combination is then all that is left.

**Faulty firmware can drive the hardware wrongly.** The firmware sets the LED currents and switches the analog multiplexers. The unmodified build is written to do this as stock firmware 2.14.1 does; changed lighting or scan code has no such reference. No current was measured in the tests.

**Do not change the option bytes.** According to GigaDevice's documentation for this family of microcontrollers (not verified by this project), switching read protection on and off again erases the whole flash including the bootloader, and the highest protection level disables the debug port permanently. `info` shows the current state of read protection; on the keyboard used for the tests it is off.

**One keyboard, one bootloader version.** Everything confirmed here comes from one Wooting 80HE with bootloader 2.3. Another keyboard may carry another bootloader version; `python3 tools/openwoot.py bootinfo` shows it without changing anything.

### Wootility

The vendor's configuration program has not been connected to a keyboard running openwoot. Three things it may do matter here:

- **It may write a "handshake" lock value.** When Wootility connects to a keyboard, it can write a lock mode to the keyboard's factory record. The firmware accepts only locking values for it, and no command of the protocol undoes the lock; [protocol](protocol.md#lock-mode) describes what a locked keyboard refuses. `info` prints the current value (`lock mode`, 0 means not locked). Whoever connects Wootility to the keyboard accepts that this value may be set.
- **It offers a restore for a keyboard it finds in update mode.** A restore installs firmware from Wooting and requests a settings reset, which erases all profiles. Keep Wootility closed while you work in update mode, and do not accept the offer unless that is what you want.
- **It may offer a firmware update.** openwoot reports the version 2.14.1. If Wooting publishes a newer version, Wootility will offer it, and that update replaces openwoot with the stock firmware.
