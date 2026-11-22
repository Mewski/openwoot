# openwoot-cli

Host tool `openwoot` for openwoot keyboards.

```sh
openwoot flash --dry-run build/wooting-80he/openwoot-wooting-80he.hex
```

| Command | Does |
| --- | --- |
| `openwoot flash IMAGE --dry-run` | reads an Intel HEX file or flat binary, finds the image descriptor, checks the image against it and prints the board, the USB ids and the blocks and CRC-24 an update would send; touches no device |
| `openwoot flash IMAGE` | the same checks, then puts the keyboard into update mode if needed (stock firmware: its reset command) and writes the image through the factory bootloader: blocks, CRC-24 comparison, completion marker, restart |
| `openwoot version` | prints the firmware and protocol version the keyboard reports over the openwoot configuration channel |

[`docs/flashing.md`](../../docs/flashing.md) describes flashing, recovery and the way back to the
stock firmware.

Device access uses `async-hid` (pure Rust: hidraw, IOHIDManager, Win32) driven by `async-io`'s
`block_on` and timers. The bootloader protocol (`src/bootloader/`) runs over the `Port` trait;
`tests/bootloader/` runs it against a fake bootloader on the host.

## Linux

`udev/70-openwoot.rules` lets the logged-in user open the keyboard's HID devices:

```sh
sudo cp udev/70-openwoot.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
```

then unplug the keyboard and plug it in again.
