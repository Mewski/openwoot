# Wooting 80HE

Board support crate for the Wooting 80HE: GD32F425 (chip HAL `openwoot-gd32f4`), key matrix of
6 rows and 17 sensor columns.

```sh
cargo xtask build wooting-80he
cargo xtask flash wooting-80he --dry-run
```

## State

Working keyboard: it enumerates at high speed, types, shows the factory profiles' effects on the
keys and the light bar, switches profiles with the Mode key, and answers `openwoot version`,
`openwoot status` and `openwoot flash` over the configuration channel.

## Memory

| Area | Addresses |
| --- | --- |
| factory bootloader, never written | `0x0800_0000`-`0x0800_7fff` |
| image (`FLASH` in `memory.x`): vector table, descriptor, code | `0x0800_8000`-`0x080f_dfff` |
| serial number mirror page, never linked | `0x080f_e000`-`0x080f_efff` |
| RAM, stack from the top | `0x2000_0000`-`0x2002_ffff` |

USB ids `31e3:1400` (product id base); update mode (factory bootloader) `31e3:140f`, blocks of
256 bytes.
