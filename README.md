# openwoot

Open keyboard firmware for Wooting hall-effect keyboards, written in Rust, with its own
configuration protocol and a host command-line tool. It keeps every feature of the stock firmware
and stays close to it on USB, so the operating system, games and the Wooting Analog SDK see the
keyboard they expect.

| Board | Microcontroller | State |
| --- | --- | --- |
| Wooting 80HE | GD32F425 | Runs as a daily keyboard: typing, LEDs, light bar, profiles and the Mode key work on hardware. Advanced keys, gamepad and XInput are tested against the stock behaviour on the host, not yet on hardware. |
| Wooting 60HE+ | GD32F303 | Builds and passes every host test; never run on hardware. Its update mode is unknown, so `openwoot flash` refuses it. |

## Features

- Hall-effect scanning with per-key actuation point, rapid trigger (normal and continuous),
  dynamic keystroke, mod-tap, toggle keys and two-key resolvers (snap tap).
- Keymap layers with an Fn layer, layout variants, 6KRO boot and N-key rollover reports, media and
  system keys, mouse keys, HID gamepad and XInput.
- Per-key RGB with every stock effect, layers, overlays, lock indicators, brightness and idle
  dimming; the light bar with profile indicators and host signals; Wooting RGB SDK commands.
- Profiles stored on the keyboard's NOR flash, switched with the Mode key or Fn + 1 to 4.
- Macros: a library of 32 macros shared by every profile (keys, media keys and mouse buttons,
  delays, text typed for the US, UK or Japanese layout, mouse movement and scrolling), played once,
  while a key is held, or toggled, from any key on any layer.
- A configuration channel of its own, shared with host software through the `openwoot-protocol`
  crate; the official Wootility still recognises the keyboard and offers to reinstall stock.
- Diagnostics over USB (`openwoot status`), no debug probe needed.

## Quick start

Requirements: [rustup](https://rustup.rs). The toolchain pinned in `rust-toolchain.toml` installs
on first use.

```sh
cargo xtask build wooting-80he          # build/wooting-80he/openwoot-wooting-80he.{elf,hex,bin}
cargo xtask flash wooting-80he          # enters update mode by itself and writes the image
cargo run -p openwoot-cli -- version    # firmware and protocol version
cargo run -p openwoot-cli -- status     # diagnostics of every subsystem
cargo run -p openwoot-gui               # desktop configurator: profiles, keys, lighting, firmware
```

**Back to stock:** open the official Wootility; it reports a required update and installs the stock
firmware (settings are reset). If the keyboard does not start, unplug it, hold the top-right key and
the Right arrow fully down, plug it in, and run the Wootility restore or `openwoot flash` again. The
factory bootloader is never written by any of these steps. Details: [docs/flashing.md](docs/flashing.md).

## Documentation

| Document | Content |
| --- | --- |
| [docs/architecture.md](docs/architecture.md) | crates, layers, tasks and real-time rules |
| [docs/building.md](docs/building.md) | toolchain, `xtask` commands and every check |
| [docs/flashing.md](docs/flashing.md) | flashing, update mode, recovery, returning to stock |
| [docs/protocol.md](docs/protocol.md) | USB face, configuration channel, endpoints, stock responder |
| [docs/storage.md](docs/storage.md) | NOR layout, settings records, device information |
| [docs/hardware.md](docs/hardware.md) | microcontrollers, peripherals and wiring per board |
| [docs/extending-keys.md](docs/extending-keys.md) | the key pipeline and how to add a key feature |
| [docs/adding-a-board.md](docs/adding-a-board.md) | what a new board needs |
| [AGENTS.md](AGENTS.md) | rules of the tree for contributors and coding agents |

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
