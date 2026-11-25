# Flashing

`openwoot flash` installs a firmware image through the factory bootloader of the keyboard. The
bootloader sits in the first 32 KiB of the microcontroller's flash, runs first after every reset
and is never written by an update, so a keyboard can always be brought back to an image that works
or to the stock firmware. This guide is about the Wooting 80HE: its bootloader and its update
protocol are known. For the Wooting 60HE+ they are not, its image descriptor names no update mode,
and `openwoot flash` refuses it before it touches a device.

The firmware runs on two Wooting 80HE units, a TenZ edition and a regular one; other units and
bootloader versions are untested. Before you flash it, practise the [recovery key
combination](#recovery-with-the-key-combination); it is the way back from an image that does not
start.

## What you need

- the keyboard on its cable, and no other keyboard of the same model connected: the tool counts
  the keyboards that match the image, in either mode, and refuses to go on if there is more than
  one;
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

  then unplug the keyboard and plug it in again. The rule gives the user of the active local
  session (`uaccess`) every HID interface of every Wooting device (vendor id `31e3`), in both
  modes. That includes the keyboard and analog interfaces, through which any program of that
  session can read every key press, and the interfaces the Wooting Analog SDK and RGB SDK use.
  Sessions over SSH get no access.

## Check an image

```sh
cargo run -p openwoot-cli -- flash --dry-run build/wooting-80he/openwoot-wooting-80he.hex
```

or `cargo xtask flash wooting-80he --dry-run`, which builds first. The dry run reads the Intel HEX
file (or a flat binary), finds the image descriptor behind the vector table and checks the image
against it: start address, size of the image area, initial stack pointer (exactly the end of the
RAM the descriptor names), every vector and the reset vector. A file ending in `.hex` is read as Intel HEX and one ending in `.bin` as a flat
binary; records may come in any order, but records that overlap or follow the end-of-file record
are refused. The descriptor names the update protocol; the tool implements one, that of Wooting's
factory bootloader (`Wooting factory bootloader`), and refuses any other value before it touches a
device. It then checks that the bootloader can take the upload: blocks of 256 bytes, an image
area that starts at `0x0800_8000`, where the bootloader writes the first block, and the image padded
to whole blocks inside that area and within the 3936 blocks below the serial page. It prints the
board, the RAM, the USB ids, the update mode with its protocol, the number of blocks and the CRC-24
that the bootloader must report after the transfer, and touches no device. The size, the block
count and the CRC change with every build:

```
board:        Wooting 80HE
image:        `build/wooting-80he/openwoot-wooting-80he.hex`, 868264 bytes at 0x0800_8000
ram:          0x2000_0000-0x2003_0000
usb ids:      31e3:1400
update mode:  31e3:140f, Wooting factory bootloader
upload:       3392 blocks of 256 bytes, CRC-24 0x0080_1d7c
dry run:      nothing was sent
```

## Flash

```sh
cargo run --release -p openwoot-cli -- flash build/wooting-80he/openwoot-wooting-80he.hex
```

The command repeats the checks of the dry run, then finds the keyboard by the USB ids in the
image descriptor:

1. **Keyboard in update mode** (`31e3:140f`, "Wooting Restore (80HE)" or "Wooting Restore",
   see [bootloader versions](#bootloader-versions)): it is used as it is.
2. **Keyboard running openwoot**: the tool reads the version over the configuration channel and
   sends the request `EnterUpdateMode`; the firmware answers and resets into update mode. An
   openwoot build that answers either request with an error, such as a build with another protocol
   version, is identified and reset with the stock feature commands 1 and 2 instead, which openwoot
   answers as the stock firmware does.
3. **Keyboard running stock firmware**: it does not answer the configuration channel (the tool
   waits 2 seconds for that), so the tool reads the firmware version with the stock feature
   command 1 and sends feature command 2 with argument 0, which resets the keyboard into update
   mode.

Either way the keyboard appears in update mode within a few seconds (the tool waits up to
20 seconds, and keeps trying to open it while the system sets up access to the new device), and
until the transfer starts nothing in flash has changed.

In update mode the tool prints what the bootloader reports (why it is in update mode, its version,
the size of its reports, the serial number, the CRC of this session; `not reported` where the
bootloader refuses a value), then:

1. unlocks the write commands and starts the upload. **From this moment the installed firmware
   counts as invalid**: until an update completes, the bootloader stays in update mode at every
   start, also after unplugging;
2. sends the blocks in ascending order, each as one 256-byte output report or four 64-byte ones
   ([bootloader versions](#bootloader-versions)) followed by the write command; a progress bar on
   stderr shows them. The bootloader erases each 4 KiB page when its first block arrives;
3. asks the bootloader for the index of the block written last and the CRC-24 over what was
   written, and compares both with the image. The bootloader verifies nothing itself; this
   comparison is the only check of the transfer;
4. only after the CRC matched, sends the reboot command with the completion marker. The
   bootloader marks the image complete and resets, and the new firmware starts. If sending this
   command ends with an error, the tool says so and still waits for the firmware, because the
   bootloader may have carried it out;
5. waits up to 20 seconds for the firmware to appear on USB. If it does not, the tool says whether
   the keyboard is still in update mode (run `openwoot flash` again) or not on USB at all. The
   first start after the stock firmware prepares the settings flash before USB starts and can take
   longer; if the keyboard never appears, use the
   [key combination](#recovery-with-the-key-combination).

A transfer of 1931 blocks took 24.4 seconds on a Wooting 80HE, about 13 ms per block, so an image
of about 3200 blocks takes about 40 seconds. Profiles and settings in the external flash are not
touched by an update.

`openwoot version` then prints the firmware and protocol version that openwoot reports over its
configuration channel (interface 2, usage page `0xff55`), and names the tool's own protocol version
when the two differ.

## Bootloader versions

Two versions of the factory bootloader are known. Both take every command as a 7-byte feature
report with report id 0 (`df db`, the command, its parameter as 4 bytes little endian), answer it
with a reply of 256 bytes (`df db`, an echo byte, the status, the body length, the body, zero
padding) in input reports, and take each 256-byte block in output reports with report id 0. They
differ in the size of those reports:

| | version 2.3 | version 2.0 |
| --- | --- | --- |
| found on | Wooting 80HE TenZ edition | Wooting 80HE |
| product string in update mode | "Wooting Restore (80HE)" | "Wooting Restore" |
| input and output reports | 256 bytes | 64 bytes |
| a reply | one input report | four input reports: the first 64 bytes of the reply, then three of zero padding |
| a block | one output report | four output reports of 64 bytes, in order |
| echo byte | the command number | always `0xff` |

The tool takes the report size from the first input report of a session, the answer to the ping:
64 or 256 bytes, any other size ends the session before anything is written. It then assembles
every reply from input reports until it holds 256 bytes, refuses a report of another size than
the first one, and sends every block as output reports of that size; both versions use one size
for input and output reports. It takes `0xff` as the echo of any command. Wootility chooses by the
size of the output report in the report descriptor, which `async-hid` does not expose: 256 bytes,
64 bytes in four parts, or, with 128 bytes, an older protocol that openwoot does not implement.
`openwoot flash` prints the size it found as `report size`.

## An interrupted update

If anything fails before the upload starts (the keyboard does not appear in update mode, cannot
be opened, or the bootloader does not answer its queries), the tool says that nothing was written.
The keyboard stays in update mode until it is unplugged; plugged in again, it starts the installed
firmware.

If anything fails after the upload started (cable pulled, a reply missing or wrong, a CRC
mismatch), the tool stops without the final command and says that the keyboard stays in update
mode. Ctrl-C ends the tool at once and without a message; the keyboard is then in the state of a
failure at the same point. Run `openwoot flash` again, with the same image or another one; the
transfer starts again from block 0, and no key needs to be held. If the bootloader stops
answering, unplug the keyboard, plug it in again (after the upload started it comes back in update
mode by itself) and run the command again.

## Recovery with the key combination

The bootloader reads two keys within the first milliseconds after power-up, before it looks at the
firmware, so this works whatever the installed image does:

1. unplug the keyboard;
2. press the **top-right key** and the **Right arrow key** all the way down and hold them;
3. plug the keyboard in while holding them; both keys must be fully down before the plug goes in;
4. the two keys light blue and the keyboard appears in update mode ("Wooting Restore (80HE)" or
   "Wooting Restore"); release the keys;
5. run `openwoot flash` with a known good image.

If the firmware starts instead, the keys were not fully down in time: unplug and try again. To
leave update mode without flashing, unplug the keyboard and plug it in without holding keys.

## Returning to the stock firmware

`openwoot flash` writes only openwoot images: it needs the image descriptor that every openwoot
build carries, and Wooting's update file has none. Return to stock with Wooting's own tool, which
recognises a keyboard running openwoot:

1. open Wootility with the keyboard plugged in (normal mode, no keys held);
2. Wootility finds the keyboard and asks for a firmware update before anything else, because
   openwoot reports itself to it as factory firmware (version 2.1.2, see
   [the identification responder](protocol.md#stock-identification-responder));
3. accept the update. Wootility resets the keyboard into update mode, installs Wooting's current
   firmware and then resets the settings, so all profiles are lost.

If Wootility does not show the keyboard or the update does not start, use the emergency route: put
the keyboard into update mode with the [key combination](#recovery-with-the-key-combination),
open Wootility, which finds it in update mode and offers to restore it, and accept the restore.

The first way is confirmed on a Wooting 80HE running openwoot: Wootility recognised the keyboard,
required its update and installed the stock firmware. The emergency route is not yet tested with
openwoot installed. Both use the same factory bootloader as `openwoot flash`, which never changes
the bootloader.

## Risks

- **The bootloader starts every complete image.** It has no test for working firmware, no
  watchdog and no time-out; an image that crashes after it was marked complete is started at every
  power-up, and the key combination is the only way back. A firmware panic resets the keyboard, so
  an image that panics at every start restarts in a loop until the key combination is used.
- **Firmware can switch the key combination off for one start**: bit 1 of the option byte that a
  reset into update mode hands to the bootloader. `openwoot flash` always sends 0 there.
- **Firmware that programs the microcontroller's flash can destroy the bootloader**: no update
  command can reach the bootloader, but firmware can. openwoot contains no code that erases or
  programs the microcontroller's flash.
- **The upper end of a write rests on the tool.** The bootloader is given block numbers, never
  addresses: block 0 always lands at `0x0800_8000`. `openwoot flash` sends only an upload that
  fits the image area of the descriptor and the 3936 blocks below the serial page at
  `0x080f_e000`; that the bootloader itself refuses a higher block number is not tested.
- **The CRC-24 is weak.** The bootloader defines it: each 16-bit word is combined with the CRC
  shifted by one bit, so some two-bit errors cancel out (bit 0 of one word with bit 1 of the next).
  It is still the only check of the transfer: the bootloader has no command that reads flash back.
- **Two keyboards, two bootloader versions.** The protocol is confirmed on a Wooting 80HE TenZ
  edition with bootloader version 2.3 and on a regular Wooting 80HE with version 2.0;
  `openwoot flash` prints the version and the report size it finds.
