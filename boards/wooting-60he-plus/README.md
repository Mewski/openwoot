# Wooting 60HE+

Board support crate for the Wooting 60HE+: GD32F303 (chip HAL `openwoot-gd32f3`), key matrix of
6 rows and 14 columns (row 0 carries no key), read through 4 analog multiplexers (16:1). This
crate holds the board facts, start-up, the configuration side and the lighting task, compiled for
size; the firmware binary `openwoot-wooting-60he-plus`, the package in `firmware/`, holds the
interrupt bindings and the key and USB tasks, compiled for speed
([speed and size](../../docs/building.md#speed-and-size)).

```sh
cargo xtask build wooting-60he-plus
cargo xtask flash wooting-60he-plus --dry-run
```

## State

The firmware builds with the same features as the Wooting 80HE (without the light bar, the host
signals, the special edition and the JIS layout) and its board data is host-tested; it has not
been confirmed on a keyboard. Without a light bar the lighting hooks are `()`: the light-bar and
host-signal endpoints answer `Unsupported` and no light-bar records are stored or loaded
([protocol](../../docs/protocol.md#configuration-model)). The update mode of this board's factory
bootloader has not been examined, so its descriptor says that the board has no update mode and
the host tool refuses to flash it.

## Memory

| Area | Addresses |
| --- | --- |
| factory bootloader, never written | `0x0800_0000`-`0x0800_6fff` |
| image (`FLASH` in `memory.x`): vector table, descriptor, code | `0x0800_7000`-`0x080f_cfff` |
| zero-wait flash (`ZERO_WAIT` in `memory.x`): all code lies in it | `0x0800_0000`-`0x0803_ffff` |
| serial number page, never linked | `0x080f_d000`-`0x080f_dfff` |
| unused page and factory device information, never linked | `0x080f_e000`-`0x080f_ffff` |
| RAM, stack from the top | `0x2000_0000`-`0x2001_7fff` |

USB ids `31e3:1320` (product id base).
