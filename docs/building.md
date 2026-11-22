# Building

This document describes how the firmware is built: what must be installed, what `make` accepts and produces, how the list of sources follows from a board's `board.mk`, which options the compiler gets, how an image is laid out in memory, and what the continuous integration does.

Related documents: [architecture](architecture.md) (the layers the build combines), [porting](porting.md) (what a board, family, driver, feature or strategy directory must contain), [flashing](flashing.md) (how a built image gets onto a keyboard), [hardware](hardware.md) (memory maps of the microcontrollers), [`third_party/README.md`](../third_party/README.md) (the libraries the firmware links).

## Prerequisites

| Tool | Version | Needed for |
| --- | --- | --- |
| GNU Arm Embedded GCC (`arm-none-eabi-gcc`, `arm-none-eabi-objcopy`, `arm-none-eabi-size`, with newlib) | GCC 10.3.1, [xPack release 10.3.1-2.3](https://github.com/xpack-dev-tools/arm-none-eabi-gcc-xpack/releases/tag/v10.3.1-2.3) | compiling and linking |
| GNU make | 3.81 or later | running the build |
| Python 3 | 3.9 or later | `make check` (`tools/check_tree.py`, standard library only) and the host tool `tools/openwoot.py`, which needs the package `hidapi` to talk to a keyboard (not to build) |

The firmware is developed and tested with GCC 10.3.1 from the xPack release named above. Other compiler versions may build, but they produce a different image, and only images of this compiler have run on a keyboard.

The xPack release is an archive that is unpacked anywhere; nothing is installed system-wide. Either put its `bin` directory on `PATH` or pass it to `make` as `TOOLCHAIN` (below). Check which compiler `make` will find:

```sh
arm-none-eabi-gcc --version
```

The Python package for the host tool is installed with:

```sh
python3 -m pip install -r tools/requirements.txt
```

## Building

All commands are run in the root of the repository. `make` builds one board per run, and the board must be named:

```sh
make boards                     # list the boards: the directories under boards/
make BOARD=wooting_80he         # build one board into build/wooting_80he/
make all-boards                 # build every board, each into build/<board>/
make check                      # the tree rules, and both images of every board that links
```

Without `BOARD`, `make` stops with `usage: make BOARD=<board>; boards: ...`; with a name that is not a directory under `boards/`, with `unknown board '<name>'`.

`make -j8 BOARD=<board>` runs eight compilers in parallel. With the compiler in a directory that is not on `PATH`:

```sh
make BOARD=wooting_80he TOOLCHAIN=/opt/xpack-arm-none-eabi-gcc-10.3.1-2.3/bin/
```

A build ends with the section sizes of the result (`arm-none-eabi-size`).

### Variables

Variables are given on the command line of `make`.

| Variable | Default | Meaning |
| --- | --- | --- |
| `BOARD` | none, required | The board to build: the name of a directory `boards/<board>/` that contains a `board.mk` |
| `LTO` | `0` | `1` adds `-flto` to compiling and linking (link-time optimisation), see [the two variants](#the-two-variants) |
| `OPT` | `-O2` | Optimisation option for compiler and linker. Only `-O2` has run on a keyboard. |
| `BUILD` | `build/<board>` | Output directory for objects and images |
| `BUILD_ROOT` | `build` | Parent of the default `BUILD` directories; `make all-boards` and the default `BUILD` use it |
| `TOOLCHAIN` | empty | Prefix in front of the tool names: the `bin` directory of the compiler **with a trailing slash**. Empty: the tools are taken from `PATH`. |
| `PYTHON` | `python3` | The interpreter that `make check` and `make flash` run |

`make` rebuilds an object when its source or a header it includes has changed (the compiler writes a `.d` file next to every object). It does not notice a changed variable. After changing `LTO`, `OPT` or the compiler, run `make BOARD=<board> clean` first, or give each configuration its own `BUILD` directory:

```sh
make BOARD=wooting_80he LTO=1 BUILD=build/wooting_80he-lto
```

### Targets

| Target | Needs `BOARD` | What it does |
| --- | --- | --- |
| `all` (default) | yes | Builds `.elf`, `.bin` and `.hex` and prints the section sizes; for a board in bring-up (`"build": "bring-up"` in its `board.json`) compiles every layer and links nothing |
| `size` | yes | Prints the section sizes of the `.elf` file, building it first if necessary |
| `hal-objects`, `driver-objects`, `board-objects`, `src-objects`, `third-party-objects` | yes | Compiles one layer of the board without linking: the family directory, the listed drivers, the board's `tables/`, the firmware in `src/` with features, strategy variants and the board's `defaults/`, the libraries |
| `flash` | yes | Builds the `.hex` file if necessary, then runs `tools/openwoot.py enter` and `tools/openwoot.py flash --yes` with it. **This replaces the firmware on the connected keyboard without asking for confirmation**; read [flashing](flashing.md) first. The host tool refuses a board whose update procedure is not known. |
| `clean` | yes | Removes the `BUILD` directory |
| `boards` | no | Lists the boards |
| `all-boards` | no | Runs `make BOARD=<board>` for every board, into `build/<board>/` |
| `check` | no | Runs `tools/check_tree.py` on the tree; then, for every board that links, builds the image with `LTO=0` and with `LTO=1` in a temporary directory (removed afterwards), prints their registration lists and checks them (`tools/check_tree.py --elf`). Fails on the first broken rule or build error. |

### Output files

All files of a board go to `build/<board>/` (or to `BUILD`).

| File | Content |
| --- | --- |
| `openwoot-<board>.elf` | The linked program with symbols and debug information; input for a debugger and for `arm-none-eabi-size`, `-nm`, `-objdump` |
| `openwoot-<board>.hex` | The image as Intel HEX. **This is the file to flash.** |
| `openwoot-<board>.bin` | The image as a flat binary for the board's `BOARD_APP_BASE`. The bytes between the vector table and the code, which no section occupies, are `FF` (`--gap-fill 0xFF`); the host tool fills the same gap of the `.hex` file with `FF`, so both describe the same flash content. |
| `openwoot-<board>.map` | The linker map: where every function and variable was placed, which input sections were discarded |
| `<path>.o`, `<path>.d` | Object and dependency file of every source, under the source's own path: `build/<board>/src/rgb/rgb_frame.o`, `build/<board>/hal/gd32f4xx/hal_scan.o`, `build/<board>/third_party/littlefs/lfs.o` |

The `build/` directory is not under version control.

## How the source list follows from `board.mk`

The [`Makefile`](../Makefile) names no board, family, driver, feature or strategy. It includes `boards/<board>/board.mk`, which sets `FAMILY`, `DEVICE_DEFS`, `DRIVERS`, `FEATURES` and one `STRATEGY_<slot>` per strategy slot, and finds everything else by its directory:

| Name in `board.mk` | Found as | The `Makefile` stops when |
| --- | --- | --- |
| `FAMILY` | `hal/<family>/hal.mk`, included after `board.mk` | the family does not exist, or `hal.mk` leaves `ARCH`, `HAL_DEFS`, `VENDOR_INCS` or `TUSB_PORT_SRC` empty |
| `DRIVERS` | `drivers/<driver>/driver.mk`, which sets `DRIVER_CLASS` | a driver does not exist, a class that exists has no driver in the list, or one class has two |
| `FEATURES` | `src/<module>/<feature>/feature.mk`, which may set `FEATURE_REQUIRES` | a feature does not exist, or a required feature is not listed |
| `STRATEGY_<slot>` | the slot `src/<module>/<slot>/strategy.mk`; the variant is a sub-directory | a slot has no selection, or the selection is not one of its sub-directories |

From these the source list is formed:

| Layer | Sources |
| --- | --- |
| Firmware | every `.c` file directly in `src/<module>/`; every `.c` file directly in the directory of each listed feature; every `.c` file directly in the directory of each selected variant; `boards/<board>/defaults/*.c` |
| Drivers | every `.c` file of each listed driver directory |
| Family | every `.c` file directly in `hal/<family>/` |
| Board tables | `boards/<board>/tables/*.c` |
| Libraries | `third_party/littlefs/lfs.c`, `lfs_util.c`; `third_party/nanopb/pb_common.c`, `pb_encode.c`, `pb_decode.c`; the peripheral library files of `VENDOR_SRC`; TinyUSB: every `.c` file of `third_party/tinyusb/src/` down to four levels except `portable/`, plus `TUSB_PORT_SRC` |

A new file in an existing module, feature, variant, driver or family needs no change to any build file. The firmware objects are linked in the sort order of their source paths, followed by the libraries. Unreferenced functions and data are removed at link time (`-ffunction-sections -fdata-sections`, `--gc-sections`), so a new file changes the image only once something calls into it.

For every feature that exists the build defines `FEATURE_<NAME>`: `1` if the board lists it, `0` otherwise. Only the feature's public header tests it ([architecture](architecture.md#features)).

### What each layer sees

The include paths are set per layer, so that the layering is enforced by the compiler:

| Layer | Include paths |
| --- | --- |
| Firmware (`src/`, `boards/<board>/defaults/`) | `boards/<board>`, `src`, `src/proto`, the selected variant directories, `drivers/include`, `hal/include`, `hal/<family>/inline`, littlefs, nanopb, TinyUSB with `src/usb` (for `tusb_config.h`) |
| Drivers | `boards/<board>`, `drivers/include`, `hal/include`, `hal/<family>/inline`, TinyUSB |
| Family | `boards/<board>`, `hal/include`, `hal/<family>/inline`, `hal/<family>`, the peripheral library (`VENDOR_INCS`), TinyUSB |
| Board tables | `boards/<board>`, `src` |
| Libraries | littlefs, nanopb, the peripheral library, TinyUSB |

### Compiler and linker options

| Option | Where | Why |
| --- | --- | --- |
| `ARCH`, e.g. `-mcpu=cortex-m4 -mthumb -mfpu=fpv4-sp-d16 -mfloat-abi=hard` | `hal/<family>/hal.mk`; compiler and linker | Both families are Cortex-M4 with a single-precision floating-point unit; `float` arguments are passed in FPU registers |
| `DEVICE_DEFS`, e.g. `-DGD32F425`, `-DGD32F30X_XD` | `board.mk`; family and libraries | Selects the device in the peripheral library |
| `HAL_DEFS` (`CFG_TUSB_MCU`, root port mode) | `hal.mk`; firmware, drivers, family, libraries | Binds TinyUSB to the family's USB controller |
| `-std=gnu11 -g` | all sources | C11 with GNU extensions; debug information in the `.elf` (it does not change the image) |
| `-fshort-enums` | all sources | An enumeration is as small as its values allow. The data structures are laid out with this option, and `_Static_assert` lines check their sizes; it must not be removed. |
| `-ffunction-sections -fdata-sections`, `-Wl,--gc-sections` | all sources, linker | Unreferenced functions and variables are left out of the image |
| `-Wall -Wextra -Wno-unused-parameter` | all sources except the libraries | The warning set of the project |
| `-fno-isolate-erroneous-paths-dereference` | firmware, drivers, family, board tables (`FW_CFLAGS`) | Some code relies on an integer division by zero giving 0, which is what the processor does while its divide-by-zero trap is off. Without the option GCC replaces a path on which it can prove a zero divisor by a trap instruction. |
| `-w` | libraries | Third-party code is compiled without warnings |
| `-DPB_NO_ERRMSG -DPB_WITHOUT_64BIT`, `-DLFS_NO_MALLOC -DLFS_NO_ASSERT -DLFS_NO_DEBUG -DLFS_NO_WARN -DLFS_NO_ERROR` | firmware and libraries | Configuration of nanopb and littlefs, see [`third_party/README.md`](../third_party/README.md) |
| `-Tboards/<board>/linker.ld -Lhal/<family>` | linker | The board's memory regions; `INCLUDE sections.ld` finds the family's section layout |
| `--specs=nosys.specs`, `-lc -lm -lnosys` | linker | The C library is the newlib of the toolchain with the stub system calls of `libnosys`. The vector table and `Reset_Handler` are the family's own (`hal/<family>/vectors.c`, `startup.c`). |

## Memory layout

A board's `linker.ld` defines the two regions; the family's `sections.ld` places the sections in them:

| Board | `FLASH` (application image) | `RAM` |
| --- | --- | --- |
| `wooting_80he` | `0x08008000`-`0x080FDFFF` (1,007,616 bytes) | `0x20000000`-`0x2002FFFF` (196,608 bytes) |
| `wooting_60he_plus` | `0x08007000`-`0x080FCFFF` (1,007,616 bytes) | `0x20000000`-`0x20017FFF` (98,304 bytes) |

The `FLASH` region starts behind the factory bootloader and ends at the page where the firmware mirrors the serial number; neither is part of the image. [Hardware](hardware.md) has the complete memory maps, [bootloader](bootloader.md) the pages above the image of the Wooting 80HE.

Order of an image in flash: `.isr_vector` (the vector table must be first: its first word is the initial stack pointer and its second the entry point, which the bootloader uses to start the image), `.text` from the next 64-byte boundary, `.rodata` with the registration lists at its end, `.ARM.extab`, `.ARM.exidx`, the constructor and destructor arrays, then the load image of `.data`, which `Reset_Handler` copies from `_sidata` to RAM (`_sdata` to `_edata`).

In RAM: `.data`, then `.bss` (`_sbss` to `_ebss`, cleared by `Reset_Handler`), then the heap growing upwards from `end`; the stack grows downwards from `_estack`, the end of RAM. Nothing limits or checks heap and stack. To see where a symbol was placed, read the `.map` file or run:

```sh
arm-none-eabi-nm -n build/wooting_80he/openwoot-wooting_80he.elf
```

## The two variants

| | `LTO=0` (default) | `LTO=1` |
| --- | --- | --- |
| How it is built | Every source file is compiled on its own. | The compiler optimises across source files at link time: it inlines and removes functions across module boundaries. |
| Symbols | Every function keeps its own symbol, so the map file and a debugger show the program as the sources describe it. | Functions that were inlined everywhere have no symbol of their own. |
| Image size | `wooting_80he` about 484 KiB, `wooting_60he_plus` about 469 KiB | about 1.5 KiB and 2.4 KiB smaller |
| On hardware | An earlier form of the Wooting 80HE firmware, built this way, ran on one keyboard. The current tree has not yet been run on hardware. | Never run on hardware |

`make check` builds both variants of every board and requires their registration lists to be the same. The sizes change with every change to the code; with the same compiler release the build is reproducible. An image is identified by what `python3 tools/openwoot.py plan <image>` prints for it: the board it fits, its size, and for a board with a known update procedure its block count and CRC.

## Third-party libraries

The firmware links the libraries in [`third_party/`](../third_party/): the GigaDevice peripheral library of the board's family (`gd32f4xx/` or `gd32f30x/`), TinyUSB (with local changes), littlefs and the nanopb runtime. [`third_party/README.md`](../third_party/README.md) lists versions, licences, the local changes and how to update a library. Of the peripheral library only the files named in `VENDOR_SRC` of the family's `hal.mk` are compiled; the library's start-up and `system_<family>.c` files are not used. The C library and the compiler support library (`libgcc`) come from the toolchain.

## Continuous integration

The workflow [`.github/workflows/build.yml`](../.github/workflows/build.yml) runs on every push and pull request. It installs the xPack release 10.3.1-2.3 for Linux (the SHA-256 of the archive is checked, [`.github/actions/arm-toolchain/action.yml`](../.github/actions/arm-toolchain/action.yml)) and:

1. lists the boards with `make -s boards`;
2. runs `make check`;
3. for every board and both variants (`LTO=0`, `LTO=1`) runs `make BOARD=<board> LTO=<0 or 1>`; for a board that links it then runs `tools/openwoot.py plan` on the `.hex` file (offline: it checks the image against the board's `board.json` and prints its size, block count and CRC) and stores the `.hex`, `.bin` and `.map` files as the artifact `openwoot-<board>-lto<0 or 1>`.

The workflow shows that the sources build, that the tree follows its rules and that each image has the shape its board expects. It does not run the firmware; nothing in it replaces a test on a keyboard.
