# Wooting 60HE+

Board support crate for the Wooting 60HE+: GD32F303 (chip HAL `openwoot-gd32f3`), key matrix of
6 rows and 14 sensor columns.

```sh
cargo xtask build wooting-60he-plus
cargo xtask flash wooting-60he-plus --dry-run
```

## State

The firmware builds with the same features as the Wooting 80HE (without the light bar) and its
board data is host-tested; it has not been confirmed on a keyboard. The update mode of this board's
factory bootloader has not been examined, so its descriptor says that the board has no update mode
and the host tool refuses to flash it.

## Memory

| Area | Addresses |
| --- | --- |
| factory bootloader, never written | `0x0800_0000`-`0x0800_6FFF` |
| image (`FLASH` in `memory.x`): vector table, descriptor, code | `0x0800_7000`-`0x080F_CFFF` |
| serial number page, never linked | `0x080F_D000`-`0x080F_DFFF` |
| unused page and factory device information, never linked | `0x080F_E000`-`0x080F_FFFF` |
| RAM, stack from the top | `0x2000_0000`-`0x2001_7FFF` |

USB ids `31e3:1320` (product id base).
