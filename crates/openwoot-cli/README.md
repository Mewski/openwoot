# openwoot-cli

Host tool `openwoot` for openwoot keyboards.

```sh
openwoot flash --dry-run build/wooting-80he/openwoot-wooting-80he.hex
```

| Command | Does |
| --- | --- |
| `openwoot flash IMAGE --dry-run` | reads an Intel HEX file or flat binary, finds the image descriptor, checks the image against it and prints the board, the USB ids and the blocks and CRC-24 an update would send; touches no device |
| `openwoot flash IMAGE` | the same checks, then puts the keyboard into update mode if needed (openwoot: the `EnterUpdateMode` request; stock firmware: its reset command) and writes the image through the factory bootloader: blocks, CRC-24 comparison, completion marker, restart |
| `openwoot version` | prints the firmware and protocol version the keyboard reports over the openwoot configuration channel |

[`docs/flashing.md`](../../docs/flashing.md) describes flashing, recovery and the way back to the
stock firmware.

Device access uses `async-hid` (pure Rust: hidraw, IOHIDManager, Win32) driven by `async-io`'s
`block_on` and timers (`hid`). The configuration channel (`channel`) and the stock feature
commands (`stock`) run over the `transfer::Reports` trait, the bootloader protocol (`bootloader`)
over its `Port` trait; `tests/it/firmware.rs` and `tests/it/bootloader.rs` run them against fakes
on the host. `keyboard` finds the keyboard, identifies its firmware and switches it into update
mode; the binary (`src/main.rs`) holds the command line and the output.

## Linux

`udev/70-openwoot.rules` lets the logged-in user open the keyboard's HID devices:

```sh
sudo cp udev/70-openwoot.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
```

then unplug the keyboard and plug it in again.
