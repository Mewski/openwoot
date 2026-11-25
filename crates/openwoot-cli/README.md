# openwoot-cli

Host tool `openwoot` for openwoot keyboards.

```sh
openwoot flash --dry-run build/wooting-80he/openwoot-wooting-80he.hex
```

| Command | Does |
| --- | --- |
| `openwoot flash IMAGE --dry-run` | reads an Intel HEX file or flat binary, finds the image descriptor, checks the image against it (initial stack pointer at the end of its RAM, vectors, reset vector), refuses an update protocol the tool does not implement, checks the image against what the factory bootloader can write, and prints the board, its RAM, the USB ids, the update mode with its protocol and the blocks and CRC-24 an update would send; touches no device |
| `openwoot flash IMAGE` | the same checks, then refuses if more than one keyboard of that model is connected, puts the keyboard into update mode if needed (openwoot: the `EnterUpdateMode` request; stock firmware, or openwoot that answers it with an error: the stock reset command) and writes the image through the factory bootloader: blocks, CRC-24 comparison, completion marker, restart |
| `openwoot version` | prints the firmware and protocol version the keyboard reports over the openwoot configuration channel |
| `openwoot status` | prints the diagnostic counters and states of every subsystem (`GetDiagnostics`) and the usage of the settings store (`GetStorageUsage`: bytes in use of the partition and the number of stored items, bytes and number of the current records) as aligned `label: value` lines |

[`docs/flashing.md`](../../docs/flashing.md) describes flashing, recovery and the way back to the
stock firmware.

Keyboard access lives in [`openwoot-host`](../openwoot-host), which the GUI shares. Device access
uses `async-hid` (pure Rust: hidraw, IOHIDManager, Win32) driven by `async-io`'s `block_on` and
timers (`hid`). The configuration channel (`channel`, `Connection`) and the stock feature commands
(`stock`) run over the `transfer::Reports` trait, the bootloader protocol (`bootloader`) over its
`Port` trait; the host crate's `tests/it/firmware.rs` and `tests/it/bootloader.rs` run them
against fakes on the host. `image` reads Intel HEX files and flat binaries; `keyboard` finds the
keyboard, identifies its firmware and switches it into update mode; `flash` checks an image
(`flash::plan`) and writes it (`flash::install`), reporting each step as an `Event`; `escape`
makes text from files and devices safe to print; `report` writes raw reports as hex for error
messages. This crate's binary holds the command line and the output: `src/main.rs` shows the plan
and the events, `status` formats the diagnostics and the storage usage.

## Linux

`udev/70-openwoot.rules` lets the user of the active local session open every HID interface of
every Wooting device, in application and update mode
([`docs/flashing.md`](../../docs/flashing.md#what-you-need) says what that includes):

```sh
sudo cp udev/70-openwoot.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
```

then unplug the keyboard and plug it in again.
