# openwoot

Open firmware for Wooting hall-effect keyboards.

openwoot replaces the firmware that runs on the keyboard, not the bootloader. For each board it behaves like that board's stock firmware 2.14.1 (same USB interfaces, same host protocol, same stored settings), and it is plain C that you can read, build and change: your own lighting effects, your own analog key features. One source tree serves every board; a board is a directory under [`boards/`](boards/).

openwoot is not affiliated with or endorsed by Wooting. It was developed by studying the behaviour of the stock firmware. Flashing it is at your own risk.

## Status

| Board | Directory | State |
| --- | --- | --- |
| Wooting 80HE | [`boards/wooting_80he`](boards/wooting_80he/) | An earlier form of this code ran on one keyboard (ANSI layout): typing, lighting, profiles, rapid trigger. The tree has been restructured since (layers, hardware abstraction, drivers, features); the restructured build has not yet been run on hardware. |
| Wooting 60HE+ | [`boards/wooting_60he_plus`](boards/wooting_60he_plus/) | Builds. Its code was run function by function on a simulated microcontroller next to stock firmware 2.14.1 of this board, with the same inputs, and gave the same results and the same sequence of register writes, including the complete start-up, apart from a short list of deliberate differences (it never writes the microcontroller's own flash, for example). That covers the logic (key processing, lighting, settings, host commands) and what the firmware asks of the hardware; it does not show that the hardware answers as assumed (pins, clocks, timing, the response of the chips). **Never run on hardware.** Its update mode has not been examined, and the host tool refuses to flash it. |

Not yet tested on hardware on any board: the `LTO=1` build, gamepad and XInput modes, the vendor's configuration program against this firmware, suspend and resume, long-term stability, ISO and JIS layouts.

## Quick start

You need `arm-none-eabi-gcc` (developed and tested with GCC 10.3.1), GNU make and Python 3.

```sh
make boards                                   # the boards that exist
make BOARD=wooting_80he                       # build/wooting_80he/openwoot-wooting_80he.hex
make check                                    # the rules of the tree, both images of every board
python3 -m pip install -r tools/requirements.txt
python3 tools/openwoot.py info                # talk to the keyboard
make BOARD=wooting_80he flash                 # enter update mode, write, verify, restart
```

Read [docs/flashing.md](docs/flashing.md) before the first flash. In short, for the Wooting 80HE:

- The update goes through the keyboard's factory bootloader, which openwoot never touches.
- If a firmware does not start: unplug, hold the top-right key and the Right arrow fully down, plug in. The keyboard starts in update mode and can be flashed again, with openwoot or with the update file from Wooting.

## Layout

```
boards/<board>/         one directory per keyboard: board.mk, board.h, wiring.h, board.json,
                        linker.ld, tables, factory defaults, key and LED positions
src/                    the firmware, the same for every board: app/, core/, one directory per
                        module, features and strategies inside their modules
drivers/                chip drivers (LED matrix driver, settings flash)
hal/                    hardware abstraction: the interface (include/hal/) and one directory per
                        microcontroller family (gd32f4xx/, gd32f30x/)
third_party/            GD32F4xx and GD32F30x libraries, TinyUSB, littlefs, nanopb
tools/                  openwoot.py (host tool: device info, update mode, flashing),
                        check_tree.py (the rules of the tree)
docs/                   documentation
```

## Documentation

| Document | Content |
| --- | --- |
| [architecture.md](docs/architecture.md) | Start here: layers, modules, HAL interface, drivers, features, strategies, registration, start-up, main loop, data flow |
| [building.md](docs/building.md) | Toolchain, `make` targets and variables, how the source list follows from `board.mk`, memory layout |
| [porting.md](docs/porting.md) | Adding a board, a microcontroller family, a driver, a feature or a strategy |
| [flashing.md](docs/flashing.md) | Updating, going back to stock, recovery, risks |
| [bootloader.md](docs/bootloader.md) | Factory bootloader: start-up decision, update protocol |
| [hardware.md](docs/hardware.md) | Microcontrollers, memory maps, peripherals, pin maps |
| [layout.md](docs/layout.md) | Key and LED positions, key ids, scan positions |
| [protocol.md](docs/protocol.md) | USB interfaces and the host configuration protocol |
| [storage.md](docs/storage.md) | Settings storage: flash layout, files, schema |
| [extending-rgb.md](docs/extending-rgb.md) | The lighting pipeline and how to add an effect |
| [extending-keys.md](docs/extending-keys.md) | The analog pipeline and how to add a key feature |
| [tools/README.md](tools/README.md) | The host tool |
| [third_party/README.md](third_party/README.md) | Versions, licences and local changes of the libraries |

## License

MIT, see [LICENSE](LICENSE). Third-party libraries keep their own licenses, see [third_party/README.md](third_party/README.md).
