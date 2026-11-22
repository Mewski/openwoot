# openwoot: notes for coding agents

Open firmware for Wooting hall-effect keyboards. Plain C11 for Cortex-M, GNU make, no RTOS. The architecture is described in `docs/architecture.md`; how to add a board, family, driver, feature or strategy in `docs/porting.md`. Read those before changing structure.

## Layout

| Directory | Holds | May include |
| --- | --- | --- |
| `src/app/` | `main`, the task loop, the tick call-back | everything below |
| `src/core/` | registration lists, start-up order, clock, soft timers, run state | `hal/` only |
| `src/<module>/` | firmware logic: `adc_scan`, `analog`, `keys`, `layout`, `hid`, `usb`, `rgb`, `profile`, `protocol`, `storage`, `tables`, `proto` | `src/core/`, other modules, `drivers/include/`, `hal/include/` |
| features (`src/rgb/signals/`, `src/rgb/lightbar/`, `src/profile/edition/`, `src/adc_scan/reference/`, `src/adc_scan/recheck/`) | optional parts a board lists in `board.mk` | like modules |
| strategies (`src/storage/device_info/`, `src/storage/layout_variant/`, `src/analog/noise_recal/`) | one variant per directory, selected in `board.mk` | like modules |
| `drivers/<chip>/` | chip drivers, written against `hal/include/` only | `hal/include/`, `drivers/include/` |
| `hal/include/hal/` | the hardware interface the firmware uses | nothing family-specific |
| `hal/<family>/` | one implementation per microcontroller family; the only place that uses a peripheral library | the family's peripheral library |
| `boards/<board>/` | `board.h` (numbers), `wiring.h` (pins), `board.json`, `board.mk`, `linker.ld`, `tables/`, `defaults/`, `layout/` | |

## Commands

```sh
make BOARD=wooting_80he              # build one board (BOARD is required); LTO=1 for the smaller variant
make boards                          # list boards
make all-boards                      # build every board
make check                           # tree rules + registration lists of the built images
python3 tools/check_tree.py --list-rules
python3 tools/openwoot.py plan build/<board>/openwoot-<board>.hex
```

Never build into `build/` of another checkout; pass `BUILD=<dir>` for scratch builds.

## Rules the tree check enforces (do not work around them)

- No `#if` on a board, family or device name anywhere in `src/`, `drivers/`, `hal/include/`.
- `FEATURE_<NAME>` appears only in that feature's public header; callers call the header, which provides do-nothing inline versions when the board does not list the feature.
- No peripheral-library header and no peripheral register address in `src/`, `drivers/`, `hal/include/`.
- No board or product name in shared code, comments included.
- Every `hal/<family>/` has the same file names and implements every function of every HAL header. Every `boards/<board>/` has the same file set and the same `board.h` constants in the same order.
- Start-up steps, profile sections and host commands are registered next to their function (`STARTUP_STEP`, `PROFILE_SECTION`, `PROTOCOL_FEATURE_COMMAND`, `PROTOCOL_REPORT_COMMAND`); order and command numbers must be unique.

## Conventions

- Behaviour stays compatible with stock firmware 2.14.1 of each board: host protocol, stored formats, order of file reads, order and values of hardware register writes. A stock defect that the host or stored data can observe is kept and marked `KNOWN ISSUE:`.
- A difference between boards is a number (`board.h`), data (`boards/<board>/tables/`), wiring (`wiring.h`), a feature, or a named strategy; never a conditional in shared code, and never a literal that happens to fit one board.
- The scan interrupt path is hot: HAL functions it calls are `static inline` in `hal/<family>/inline/`.
- Every header starts with a comment saying what the unit is for and what it depends on. No commented-out code.
- `src/proto/settings.pb.c` and `.pb.h` are generated; edit `settings.proto` and regenerate with the command in `src/proto/README.md`.
- The microcontroller's own flash below the application start holds the factory bootloader. Code must never erase or program it. Boards whose `board.json` says updates are not supported must stay that way until their update mode is understood.
