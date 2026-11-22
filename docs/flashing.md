# Flashing

`openwoot flash` installs a firmware image through the factory bootloader of the keyboard. The
bootloader sits in the first 32 KiB of the microcontroller's flash, runs first after every reset
and is never written by an update, so a keyboard can always be brought back to an image that works
or to the stock firmware. This guide is about the Wooting 80HE: its bootloader and its update
protocol are known. For the Wooting 60HE+ they are not, its image descriptor names no update mode,
and `openwoot flash` refuses it before it touches a device.

**The Rust firmware has not yet run on hardware.** Before you flash it, practise the
[recovery key combination](#recovery-with-the-key-combination); it is the way back from an image
that does not start.

## What you need

- the keyboard on its cable, and no other Wooting keyboard connected (the tool uses the first one
  it finds);
- a second keyboard: in update mode the keyboard does not type;
- Wootility closed, including a browser tab with its web version: two programs that talk to the
  same HID interface read each other's replies;
- an image, built with `cargo xtask build wooting-80he` (see [building](building.md)):
  `build/wooting-80he/openwoot-wooting-80he.hex`;
- on Linux, the udev rule that lets the logged-in user open the keyboard's HID devices:

  ```sh
  sudo cp crates/openwoot-cli/udev/70-openwoot.rules /etc/udev/rules.d/
  sudo udevadm control --reload-rules
  ```

  then unplug the keyboard and plug it in again.

## Check an image

```sh
cargo run -p openwoot-cli -- flash --dry-run build/wooting-80he/openwoot-wooting-80he.hex
```

or `cargo xtask flash wooting-80he --dry-run`, which builds first. The dry run reads the Intel HEX
file (or a flat binary), finds the image descriptor behind the vector table and checks the image
against it: start address, size of the image area, initial stack pointer, every vector and the
reset vector. It prints the board, the USB ids, the number of 256-byte blocks and the CRC-24 that
the bootloader must report after the transfer, and touches no device. The size, the block count
and the CRC change with every build:

```
board:        Wooting 80HE
image:        `build/wooting-80he/openwoot-wooting-80he.hex`, 3104 bytes at 0x0800_8000
usb ids:      31e3:1400
update mode:  31e3:140f, 13 blocks of 256 bytes, CRC-24 0x0092_33f3
dry run:      nothing was sent
```

## Flash

```sh
cargo run --release -p openwoot-cli -- flash build/wooting-80he/openwoot-wooting-80he.hex
```

The command repeats the checks of the dry run, then finds the keyboard by the USB ids in the
image descriptor:

1. **Keyboard in update mode** (`31e3:140f`, "Wooting Restore (80HE)"): it is used as it is.
2. **Keyboard running stock firmware**: the tool reads the firmware version with the stock feature
   command 1 and sends feature command 2 with argument 0, which resets the keyboard into update
   mode; the keyboard appears in update mode within 2 to 9 seconds (the tool waits up to
   20 seconds). Until the transfer starts nothing in flash has changed.
3. **Keyboard running openwoot**: the configuration protocol has no request for update mode yet,
   so the tool stops with an error that says so. Use the
   [key combination](#recovery-with-the-key-combination) to enter update mode, then run the
   command again.

In update mode the tool prints what the bootloader reports (why it is in update mode, its version,
the serial number, the CRC of this session), then:

1. unlocks the write commands and starts the upload. **From this moment the installed firmware
   counts as invalid**: until an update completes, the bootloader stays in update mode at every
   start, also after unplugging;
2. sends the blocks in ascending order, each as one 256-byte output report followed by the write
   command; a progress bar on stderr shows them. The bootloader erases each 4 KiB page when its
   first block arrives;
3. asks the bootloader for the index of the block written last and the CRC-24 over what was
   written, and compares both with the image. The bootloader verifies nothing itself; this
   comparison is the only check of the transfer;
4. only after the CRC matched, sends the reboot command with the completion marker. The
   bootloader marks the image complete and resets, and the new firmware starts;
5. waits up to 20 seconds for the firmware to appear on USB.

A transfer of about 1900 blocks takes about 25 seconds. Profiles and settings in the external
flash are not touched by an update.

`openwoot version` then prints the firmware and protocol version that openwoot reports over its
configuration channel (interface 2, usage page `0xff55`).

## An interrupted update

If anything fails after the upload started (cable pulled, a reply missing or wrong, a CRC
mismatch), the tool stops without the final command and says that the keyboard stays in update
mode. Run `openwoot flash` again, with the same image or another one; the transfer starts again
from block 0, and no key needs to be held. If the bootloader stops answering, unplug the keyboard,
plug it in again (it comes back in update mode by itself) and run the command again.

## Recovery with the key combination

The bootloader reads two keys within the first milliseconds after power-up, before it looks at the
firmware, so this works whatever the installed image does:

1. unplug the keyboard;
2. press the **top-right key** and the **Right arrow key** all the way down and hold them;
3. plug the keyboard in while holding them; both keys must be fully down before the plug goes in;
4. the two keys light blue and the keyboard appears as "Wooting Restore (80HE)"; release the keys;
5. run `openwoot flash` with a known good image.

If the firmware starts instead, the keys were not fully down in time: unplug and try again. To
leave update mode without flashing, unplug the keyboard and plug it in without holding keys.

## Returning to the stock firmware

`openwoot flash` writes only openwoot images: it needs the image descriptor that every openwoot
build carries, and Wooting's update file has none. Return to stock with Wooting's own tool:

1. put the keyboard into update mode with the [key combination](#recovery-with-the-key-combination);
2. open Wootility; it finds the keyboard in update mode and offers to restore it;
3. accept the restore. It installs Wooting's firmware and also resets the settings, so all
   profiles are lost.

The restore through Wootility is not yet tested on hardware with openwoot installed. It uses the
same factory bootloader as `openwoot flash`, which never changes the bootloader.

## Risks

- **The bootloader starts every complete image.** It has no test for working firmware, no
  watchdog and no time-out; an image that crashes after it was marked complete is started at every
  power-up, and the key combination is the only way back.
- **Firmware can switch the key combination off for one start**: bit 1 of the option byte that a
  reset into update mode hands to the bootloader. `openwoot flash` always sends 0 there.
- **Firmware that programs the microcontroller's flash can destroy the bootloader**: no update
  command can reach the bootloader, but firmware can. openwoot writes only the serial page, as the
  stock firmware does.
- **One keyboard, one bootloader version.** The protocol is confirmed on a Wooting 80HE with
  bootloader version 2.3; `openwoot flash` prints the version it finds.
