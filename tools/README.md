# Tools

| Tool | Purpose |
| --- | --- |
| [`openwoot.py`](openwoot.py) | Host tool: talks to the keyboard over USB HID. Reads identification and state from the running firmware, sends the keyboard into update mode and writes a firmware image through the factory bootloader. |
| [`check_tree.py`](check_tree.py) | Checks the rules of the source tree and the registration lists of built images; run by `make check` |
| [`udev/70-openwoot.rules`](udev/70-openwoot.rules) | Linux: lets the logged-in user open the keyboard's HID devices |

Background: [flashing](../docs/flashing.md) describes the image and the update procedure, [bootloader](../docs/bootloader.md) the factory bootloader and its commands, [protocol](../docs/protocol.md) the commands of the running firmware, [building](../docs/building.md) `make check`.

## `check_tree.py`

Needs Python 3.9 or later, nothing else.

```sh
python3 tools/check_tree.py                     # the rules of the tree
python3 tools/check_tree.py --list-rules        # print the rules
python3 tools/check_tree.py --elf build/wooting_80he/openwoot-wooting_80he.elf
```

Without arguments it checks the tree it is in and prints one line per violation; the exit status is 1 if there is one. With `--elf` it reads the registration lists (start-up steps, profile sections, feature commands, report commands) of the given images, prints them, and checks that every list is present and non-empty, that no order or command number occurs twice in a list, and, if several images of one board are given, that they have the same lists. [Architecture](../docs/architecture.md#layers) explains the rules.

## `openwoot.py`

Needs Python 3.9 or later and the package `hidapi` (only to talk to a keyboard; `plan` works without it).

The update sequence (`enter`, `flash`) and the recovery key combination are confirmed on a Wooting 80HE, from a macOS host. The tool uses no platform-specific calls and is meant to run on Linux and Windows as well; that is not yet tested.

### Boards

The tool has no list of keyboards of its own. Every board is described by its `boards/<board>/board.json` ([porting](../docs/porting.md#boardjson)): USB ids of the running firmware, image limits, and whether the board's update procedure is known (then also the product id of the update mode and the block size). A connected keyboard is recognised by its USB ids, an image by its start address and its initial stack pointer. `--board NAME` (a directory name under `boards/`, given before the sub-command) names the board where the image does not tell, as for a flat binary.

`enter` and `flash` refuse a board whose update procedure is not known, before anything is sent:

```
openwoot: update mode and flashing are not available for the Wooting 60HE+: the update mode of this board has not been examined; nothing was changed
```

At present the update procedure is known for the Wooting 80HE only; everything below about update mode, flashing and recovery applies to that board.

### Installation

From the root of the repository:

```sh
python3 -m venv .venv
. .venv/bin/activate
pip install -r tools/requirements.txt
```

On Windows the second line is `.venv\Scripts\activate`. All examples below assume the virtual environment is active and the current directory is the root of the repository.

**Linux:** a normal user may not open the keyboard's HID devices until a udev rule allows it. Install the rule once, then unplug the keyboard and plug it in again:

```sh
sudo cp tools/udev/70-openwoot.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
```

Connect one Wooting keyboard at a time: the tool uses the first one it finds. Close Wootility while you use the tool, so that the two do not read each other's replies.

### The two modes

The keyboard answers in one of two modes, and every sub-command needs one of them:

| Mode | What answers | USB identity | Sub-commands |
| --- | --- | --- | --- |
| running firmware | openwoot or the stock firmware | vendor id and one of the three product ids (`product_id_base` + USB mode 0..2) of a `board.json`; HID interface with usage page `0xFF55` | `info`, `raw`, `enter` |
| update mode | the factory bootloader | vendor id and `update.product_id` of a `board.json` (Wooting 80HE: `0x140F`, product name "Wooting Restore (80HE)"); usage page `0xFFFF` | `bootinfo`, `flash`, `reboot` |

`plan` works without a keyboard. `combo` watches the keyboard change mode.

**In update mode the keyboard does not type.** It stays there, without time-out, until `flash` has finished, `reboot` is sent or the cable is pulled. If it is your only keyboard, start `enter` and `flash --yes` in one command line (see [A flashing session](#a-flashing-session)).

If a sub-command finds the keyboard in the wrong mode it says so and changes nothing:

```
openwoot: the keyboard runs its firmware, this command needs update mode; run "openwoot.py enter" first
openwoot: the keyboard is in update mode, this command needs the running firmware; flash an image, or leave update mode with "openwoot.py reboot"
```

### Sub-commands

```
python tools/openwoot.py [--log FILE] [--board NAME] COMMAND [ARGUMENT ...]
```

`python tools/openwoot.py --help` lists the sub-commands, `python tools/openwoot.py COMMAND --help` describes one. The exit status is 0 on success, 1 after a failure (one line starting with `openwoot:` on standard error) and 2 for a wrong command line.

#### `info`

Queries the running firmware. Sends only commands that read.

```
$ python tools/openwoot.py info
device        : Wooting 80HE, product id 0x1402, serial <serial>
firmware      : 2.14.1
usb speed     : high
mcu unique id : <24 hex digits>
spi flash     : manufacturer 0xef, device 0x16, unique id <16 hex digits>
device config : 0000000011000c (MCU read protection off)
active profile: index 0, namespace 0
profiles      : 4
lock mode     : 0
```

A line is left out if the firmware does not answer that query with status `0x88`. The serial number and the two unique ids identify one keyboard; remove them before you post the output somewhere.

#### `raw CMD [ARG]`

Sends one feature command with a 32-bit argument (default 0) to the running firmware and prints the status and the payload of the reply. Numbers may be decimal or hexadecimal (`0x45`).

```
$ python tools/openwoot.py raw 69
usb speed: status 0x88 (ok), 1 bytes: 03
$ python tools/openwoot.py raw 62 0
profile count: status 0x88 (ok), 1 bytes: 04
```

Only commands that neither write to flash nor change the state of the keyboard are sent:

| Command | Name | Command | Name |
| --- | --- | --- | --- |
| 0 | ping | 58 | storage usage |
| 1 | version | 62 | profile count |
| 3 | serial | 69 | usb speed |
| 11 | active profile | 73 | active profile state |
| 19 | device config | 81 | lock mode |

Any other number is refused before anything is sent:

```
$ python tools/openwoot.py raw 23
openwoot: feature command 23 is not in the read-only list of this tool (0, 1, 3, 11, 19, 58, 62, 69, 73, 81)
```

Status values: `0x88` ok, `0xFF` error, `0x66` unknown command, `0x77` queue full, `0xAA` unsupported. [Protocol](../docs/protocol.md) has the command table.

#### `enter`

Asks the running firmware to reset into update mode (feature command 2, argument 0) and waits up to 20 seconds for the bootloader to appear. If the keyboard is in update mode already, the command says so and succeeds. Refused for a board whose update procedure is not known.

```
$ python tools/openwoot.py enter
firmware version 2.14.1
asking the firmware to reset into update mode (feature command 2, argument 0)
update mode: 31e3:140f "Wooting Restore (80HE)"
```

Nothing in flash changes. `reboot` returns to the firmware.

#### `bootinfo`

Update mode. Sends the bootloader's read-only commands and changes nothing.

```
$ python tools/openwoot.py bootinfo
update mode   : 31e3:140f "Wooting Restore (80HE)"
flags         : 0x02 (update mode requested by the firmware)
bootloader    : version 2.3
serial        : <serial>
session crc   : 0x000000 (blocks written since the bootloader started)
```

`flags` tells why the bootloader stayed in update mode, see [Bootloader flags](#bootloader-flags). `session crc` is the CRC-24 over the blocks written since the bootloader started; before an upload it is 0. It does not check firmware that is already installed.

#### `plan IMAGE`

Offline. Loads an image the way `flash` does, finds the board it is built for, runs the checks and prints what an update would send: for a board with a known update procedure the number of blocks and the CRC-24 that the bootloader must report after the upload.

```
$ python tools/openwoot.py plan build/wooting_80he/openwoot-wooting_80he.hex
board         : Wooting 80HE (boards/wooting_80he)
image         : build/wooting_80he/openwoot-wooting_80he.hex, <size> bytes after padding
blocks        : <blocks> of 256 bytes
crc           : <crc>
```

For a board whose update procedure is not known, `plan` prints the size and `update : not supported: ...` instead of blocks and CRC. The numbers depend on the build. `IMAGE` is the Intel HEX file of a build; this is the file to flash. A flat binary for the board's start address is accepted too; the `.bin` and the `.hex` file of a build describe the same flash content and give the same CRC. The CRC is always that of the bytes that would be written. Rules applied to the image:

| Step | Rule |
| --- | --- |
| Format | a file that starts with a colon is read as Intel HEX, anything else as a flat binary |
| Intel HEX | every record needs a correct checksum; an end-of-file record must be present; space between records is filled with `FF` |
| Board | the board whose `image.base` is the first address of the image and whose `image.stack_top` is its first word; `--board` names it instead (a flat binary has no address) |
| Padding | for a board with a known update procedure, the image is filled with `FF` to a multiple of `update.block_size` |
| Size | at most `image.end` - `image.base` bytes (Wooting 80HE: 3936 blocks, `0x08008000`-`0x080FDFFF`); the bootloader writes nothing beyond |
| First word | the initial stack pointer must be the board's `image.stack_top`, the end of RAM |
| Second word | the reset vector must be odd (Thumb code) and point into the image, behind the vector table (`image.vectors` entries) |

An image that fails a check is refused with a message such as `openwoot: initial stack pointer 0x00000000, expected 0x20030000`.

#### `flash IMAGE`

Update mode. Writes the image, verifies it and starts it. Refused for a board whose update procedure is not known, and when the image is for another board than the keyboard in update mode.

```
$ python tools/openwoot.py flash build/wooting_80he/openwoot-wooting_80he.hex
board         : Wooting 80HE (boards/wooting_80he)
image         : build/wooting_80he/openwoot-wooting_80he.hex, <size> bytes after padding
blocks        : <blocks> of 256 bytes
crc           : <crc>
update mode   : 31e3:140f "Wooting Restore (80HE)"
flags         : 0x02 (update mode requested by the firmware)
bootloader    : version 2.3
serial        : <serial>
session crc   : 0x000000 (blocks written since the bootloader started)
type ERASE to invalidate the installed firmware and write <blocks> blocks: ERASE
command 6: unlock the write commands
command 2: start upload of <blocks> blocks
  block 1 / <blocks>
  block 101 / <blocks>
  ...
all blocks written in <seconds> s
CRC verified  : <crc>
command 5 with bit 0: write the completion marker and start the firmware
firmware is back: 31e3:1402 "Wooting 80HE"
```

The steps, in this order:

1. The image is loaded and checked as by `plan`. Nothing has been sent yet.
2. The read-only queries of `bootinfo`.
3. The confirmation: type the word `ERASE` and press Enter. Any other answer ends the command with nothing changed. `--yes` skips the question.
4. Command 6 unlocks the write commands. It erases nothing.
5. Command 2 starts the upload. It erases the completion marker at `0x080FFFFC`: from here on the installed firmware counts as invalid, and the bootloader stays in update mode after every restart until an upload has been completed.
6. Each 256-byte block is sent as one output report and programmed with command 3, in ascending order.
7. Command 8 must report the last block of the image as the block written last, and command 4 must report the CRC that `plan` printed. The bootloader itself checks nothing; this comparison is the only proof that the image arrived.
8. Only after the CRC matched, command 5 with bit 0 writes the completion marker and restarts the keyboard. The tool waits up to 20 seconds for the firmware to appear on USB.

If one of the steps 4 to 7 fails, the message ends with `The keyboard stays in update mode; run the flash command again.` Do exactly that. The upload always starts again from block 0. An upload of about 1930 blocks takes about 25 seconds.

If the firmware does not appear after step 8, the image arrived intact but does not start. Get back into update mode with the [key combination](#recovery-with-the-key-combination) and flash another image.

#### `reboot [0|1]`

Update mode. Sends command 5; the keyboard restarts.

```
$ python tools/openwoot.py reboot
reboot requested (command 5, parameter 0)
```

| Parameter | Effect |
| --- | --- |
| `0` (default) | Restart only; flash is not touched. The installed firmware starts, unless an upload was begun and not completed: then the bootloader comes back in update mode. |
| `1` | Write the completion marker first, without any check, then restart. `flash` does this itself after the CRC matched. By hand it marks whatever is in flash as complete firmware; do not use it after a failed upload. |

#### `combo`

Checks the recovery key combination without writing anything. The command waits for the keyboard to be unplugged and to come back, reports the mode it came back in, and if that is update mode, leaves it again with command 5, parameter 0.

```
$ python tools/openwoot.py combo
waiting up to 600 s for the keyboard to be unplugged
keyboard unplugged; press the top-right key and the Right arrow fully, then plug it in
update mode   : 31e3:140f "Wooting Restore (80HE)"
flags         : 0x01 (recovery key combination held)
bootloader    : version 2.3
serial        : <serial>
session crc   : 0x000000 (blocks written since the bootloader started)
result: update mode; key combination DETECTED by the bootloader
release the keys; leaving update mode in 6 s (command 5, parameter 0, writes no flash)
firmware is back: 31e3:1402 "Wooting 80HE"
```

If the keys were not down in time, the firmware starts and the result line reads `result: the firmware started (...); the key combination was NOT detected`.

| Option | Meaning |
| --- | --- |
| `--wait SECONDS` | time allowed for the unplug and the replug together (default 600) |
| `--hold SECONDS` | time spent in update mode before leaving it (default 6); release the keys during this time |

#### `--log FILE`

Given before the sub-command, it appends every request and reply with a timestamp to `FILE`, together with the lines the tool prints (marked `#`):

```
$ python tools/openwoot.py --log update.log bootinfo
$ cat update.log
16:18:02.159 opened update mode 31e3:140f "Wooting Restore (80HE)"
16:18:02.159 # update mode   : 31e3:140f "Wooting Restore (80HE)"
16:18:02.160 -> feature dfdb0000000000
16:18:02.198 <- input   dfdb008802ff020000000000000000000000000000000000
16:18:02.198 # flags         : 0x02 (update mode requested by the firmware)
...
```

The log contains the serial number of the keyboard.

### A flashing session

Wooting 80HE. Build the firmware, send the keyboard into update mode, write the image:

```sh
make BOARD=wooting_80he
python tools/openwoot.py enter
python tools/openwoot.py flash build/wooting_80he/openwoot-wooting_80he.hex
```

`flash` asks for the word `ERASE`, and the keyboard that is being updated cannot type it; with a second keyboard this is no problem. `make BOARD=wooting_80he flash` builds the firmware and then runs the two commands in one go, with `flash --yes`: it does not ask, and the update starts as soon as the keyboard is in update mode. Without `make`, give both commands in one line and skip the question:

```sh
python tools/openwoot.py enter && python tools/openwoot.py flash --yes build/wooting_80he/openwoot-wooting_80he.hex
```

`python tools/openwoot.py plan IMAGE` shows beforehand, without a keyboard, what `flash` will send.

When `flash` ends with `firmware is back`, the new firmware is running. An update writes the firmware area of the microcontroller only; the settings stored in the keyboard stay as they are.

If `flash` fails or is interrupted, the keyboard stays in update mode, also after it is unplugged and plugged in again. Run `flash` again; nothing else is needed. To return to the stock firmware, flash the update file from Wooting; see [flashing](../docs/flashing.md).

### Recovery with the key combination

Wooting 80HE. The bootloader can be told to stay in update mode without help from the installed firmware. It reads two keys directly after power-on, before it starts any firmware:

1. Unplug the keyboard.
2. Press the top-right key and the Right arrow key all the way down and keep them down.
3. Plug the keyboard in. Both keys must be fully down before the plug goes in.
4. The two keys light blue and the keyboard appears on USB as "Wooting Restore (80HE)". Release the keys.
5. `python tools/openwoot.py bootinfo` shows the flags `0x01`. Now `flash` an image, or leave update mode with `python tools/openwoot.py reboot`.

If the keyboard starts normally instead, the keys were not fully down in time: unplug it and try again. `combo` lets you practise this without any risk.

Confirmed on a Wooting 80HE with working firmware installed. With firmware that does not start, the procedure is not yet tested on hardware; it does not depend on the firmware, because the bootloader reads the keys before any firmware code runs.

### Bootloader flags

Wooting 80HE. After every reset the bootloader decides once whether it starts the firmware. `bootinfo` shows its reasons as a flag byte:

| Bit | Value | Printed as | Meaning |
| --- | --- | --- | --- |
| 0 | `0x01` | recovery key combination held | The top-right key and the Right arrow were fully pressed when the keyboard was powered. |
| 1 | `0x02` | update mode requested by the firmware | The firmware reset the keyboard with feature command 2, which is what `enter` sends. The request holds for one start. |
| 2 | `0x04` | no valid firmware installed | The first word of the firmware area at `0x08008000` is not a stack pointer in RAM, or the completion marker at `0x080FFFFC` is not written: an upload was begun with command 2 and not completed with command 5, bit 0. Not yet tested on hardware. |

Several flags can be set at once. With no flag set the bootloader starts the firmware. With any flag set it stays in update mode, without time-out, until command 5 (`reboot`, or the end of `flash`) or until power is removed.
