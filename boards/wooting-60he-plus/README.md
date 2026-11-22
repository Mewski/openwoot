# Wooting 60HE+

Board support crate for the Wooting 60HE+: GD32F303 (chip HAL `openwoot-gd32f3`), key matrix of
6 rows and 14 sensor columns.

```sh
cargo xtask build wooting-60he-plus
cargo xtask flash wooting-60he-plus --dry-run
```

## State

Scaffold: the image links and carries its descriptor, the firmware does nothing yet. The update
mode of this board's factory bootloader has not been examined, so its descriptor says that the
board has no update mode and the host tool refuses to flash it.

## Memory

| Area | Addresses |
| --- | --- |
| factory bootloader, never written | `0x08000000`-`0x08006FFF` |
| image (`FLASH` in `memory.x`): vector table, descriptor, code | `0x08007000`-`0x080FCFFF` |
| serial number page, never linked | `0x080FD000`-`0x080FDFFF` |
| unused page and factory device information, never linked | `0x080FE000`-`0x080FFFFF` |
| RAM, stack from the top | `0x20000000`-`0x20017FFF` |

USB vendor id `0x31E3`, product id base `0x1320`.
