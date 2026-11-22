# Wooting 80HE

Board support crate for the Wooting 80HE: GD32F425 (chip HAL `openwoot-gd32f4`), key matrix of
6 rows and 17 sensor columns.

```sh
cargo xtask build wooting-80he
cargo xtask flash wooting-80he --dry-run
```

## State

Scaffold: the image links and carries its descriptor, the firmware does nothing yet. Do not flash
it.

## Memory

| Area | Addresses |
| --- | --- |
| factory bootloader, never written | `0x08000000`-`0x08007FFF` |
| image (`FLASH` in `memory.x`): vector table, descriptor, code | `0x08008000`-`0x080FDFFF` |
| serial number mirror page, never linked | `0x080FE000`-`0x080FEFFF` |
| RAM, stack from the top | `0x20000000`-`0x2002FFFF` |

USB vendor id `0x31E3`, product id base `0x1400`; update mode (factory bootloader) product id
`0x140F`, 256-byte blocks.
