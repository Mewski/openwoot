# Storage

What the firmware keeps across a power cycle, where it lies, how it is written and what survives a power cut. The code is `openwoot::storage` (`crates/openwoot/src/storage/`), the flash driver `openwoot_drivers::spi_nor::SpiNor`, the bus `hal::spi` of each chip HAL, and the board facts `boards/<board>/src/storage.rs`.

## Where data lives

Both boards carry an 8 MiB SPI NOR flash (W25Q64 or GD25Q64 class, 4096-byte sectors, 256-byte pages).

| Medium | Wooting 80HE | Wooting 60HE+ |
| --- | --- | --- |
| settings partition | NOR sectors 0 to 127 (`0x00_0000`-`0x07_ffff`, 512 KiB) | the same |
| unused, never touched | NOR sectors 128 to 2046 | NOR sectors 128 to 2047 |
| factory device-information record | NOR sector 2047 (`0x7f_f000`), read only | microcontroller flash page `0x080f_f000`, read only |
| physical layout | field of the device-information record | bit 0 of the user option byte (`FMC_OBSTAT` bits 17..10) |
| serial text for the factory bootloader | microcontroller flash page `0x080f_e000`, read for the USB serial string, never written | page `0x080f_d000`, read, never written |

`NorLayout` (in `boards/<board>/src/storage.rs`) states the chip size, the settings partition and the device-information sector. `NorLayout::settings_region` is evaluated in a constant and fails the build unless the partition is whole sectors, at least two, inside the chip and apart from the device-information sector. `Partition` receives only that region and refuses every read, program or erase outside it, so the settings store cannot reach the device-information sector even through a bug in the store.

## Bus and driver

`hal::spi::Spi` is the SPI master: mode 0, 8 bit, most significant bit first, software chip select, clock = bus clock / 2 (80HE: `SPI0` at 50 MHz on PB3, PA7, PB4; 60HE+: `SPI1` at 30 MHz on PB13, PB15, PB14). Every transfer runs by DMA in both directions; the receive channel's interrupt (priority 1) wakes the waiting task, and the completion is also visible in the DMA flags, so the same code works under `embassy_futures::block_on` at start-up. Dropping a transfer aborts it. Bytes sent while reading are `0x00`.

| Board | Receive | Transmit | Interrupt |
| --- | --- | --- | --- |
| Wooting 80HE | `DMA1` channel 2, sub-peripheral 3 | `DMA1` channel 3, sub-peripheral 3 | `DMA1_Channel2` |
| Wooting 60HE+ | `DMA0` channel 3 | `DMA0` channel 4 | `DMA0_Channel3` |

`hal::spi::Device` adds the chip-select pin (PD2 on the 80HE, PB12 on the 60HE+): low for one `embedded_hal_async::spi::SpiDevice` transaction, high again afterwards, also when the transaction is dropped.

`SpiNor` implements `embedded_storage_async::nor_flash::NorFlash` (read size 1, write size 4, erase size 4096) and `MultiwriteNorFlash` with these commands:

| Command | Use |
| --- | --- |
| `0x05` read status | before every command until bit 0 (busy) is clear, and after every page program and sector erase, so a finished future means a finished flash operation; the task yields between polls |
| `0x06` write enable | before every page program and sector erase |
| `0x0b` fast read | 24-bit address, one dummy byte `0xff` |
| `0x02` page program | split at 256-byte page boundaries |
| `0x20` sector erase | one 4096-byte sector |
| `0x90` manufacturer and device id | address 0, two bytes |
| `0x4b` unique id | address 0, four dummy bytes `0xff`, eight bytes (so the answer equals stock's) |

## Settings store

The settings are a `sequential-storage` 8.0.2 map (`storage::Settings`) on the partition, with all three caches (page states and pointers for 128 sectors, pointers for 256 keys).

- Key: 16 bits, the slot in the high byte (`Scope::Profile(n)` = `n`, `Scope::Global` = `0xff`) and the item in the low byte (`RecordKey::new`, `RecordKey::of::<R>`).
- Value: the record version (one byte) followed by the record in postcard. A record type implements `storage::Record` with its `ITEM` and `VERSION`; at most 2048 bytes.
- Versions: a record whose version differs from its type's `VERSION`, or one that does not decode, counts as absent and the caller uses its default.
- Loading at start-up: `Settings::visit` walks every stored item once (several items per key may appear; the last one is current) and `Stored::decode` checks the version; single lookups (`Settings::load`) scan the used area when a key is absent, so the start-up path loads everything in one pass.
- `Settings::store`, `Settings::remove` (marks the item erased; slow, scans the used area) and `Settings::factory_reset` (erases the partition, then writes the format record).
- In place, for records too large to pass around on the stack: `Settings::load_with(key, read)` hands the stored version and body to `read` without decoding, and `Settings::store_with(key, version, encode)` lets `encode` write the body straight into the store's buffer (`None` or a length beyond the buffer stores nothing). `profile::Model` decodes and encodes its parts this way, from and into the shared state of the running profile, one keymap or colour layer at a time.
- Placement: the store (2 KB buffer and the page and key cache) is never moved. A board keeps an empty `Settings::new()` in a static (`storage::SETTINGS_STORE`), `Settings::open` fills it in place, and `storage::Storage` and `profile::Model` hold `&mut Settings`.

Global item 0 is the format record (`Format { generation: 1 }`). `Settings::open` erases the partition and writes it when it is missing or different (first boot, the stock littlefs volume, a future incompatible store) and reports `Opened::Formatted(FormatReason::Unrecognised)`; a format request of the factory bootloader (`0x420` in backup register 1, taken by `Bootloader::take_format_request`) erases it too (`FormatReason::Requested`). A read error of the flash is reported and changes nothing.

Switching from stock to this firmware therefore replaces the stock littlefs volume in sectors 0 to 127 with this store; the device-information sector is untouched. Switching back, the stock firmware finds no littlefs volume, formats the chip and starts with factory settings.

## Device information

The factory record is read once at start-up through a `storage::DeviceInfoSource` (`storage::NorSector` on the 80HE, `storage::McuPage` on the 60HE+) into `storage::DeviceInfo`: the serial number (`SerialNumber`, printed by `SerialNumber::text` as `A<supplier>B<year><week>W<product>[T<pcb design>]<revision>[<minor revision>][S<edition>]<stage letter><product id>`), the auxiliary record (`AuxRecord`, at most 58 bytes, kept as bytes), the physical layout and the lock mode (kept as its number).

| Marker at `+0xffc` | Length prefix | Largest payload |
| --- | --- | --- |
| `0x1337` | 1 byte | 254 bytes |
| `0x1338` | 2 bytes, little endian | 4090 bytes |
| none, first 30 bytes `0xff` | | blank |
| none, otherwise | 80HE: 1 byte (`Unmarked::ShortFormat`); 60HE+: the older raw record (`Unmarked::RawRecord`) | |

The payload is the protobuf message `DeviceInfo` of the stock schema (field 1 serial number with required fields 1 to 7, 2 auxiliary record, 3 physical layout, 4 lock mode); a missing required field, a wrong wire type or a value wider than its field makes the record undecodable. The raw record of the 60HE+ is the serial number without its optional fields, a length byte at offset 12 and an encoded auxiliary record at offset 13. A length beyond the 44 bytes of the raw record means "no auxiliary record"; the bytes behind the record are never decoded.

The firmware never writes the device information on either board. Deviation from stock on the 80HE: host commands that change the serial number, the auxiliary record, the layout or the lock mode cannot store them there; a later package keeps such values in the settings store if they are needed.

The physical layout is a `storage::LayoutVariantSource`: `InDeviceInfo` (80HE: the record's field, 0 when absent) or `OptionBit` (60HE+: bit 0 of the user option byte, read by `hal::flash::user_option`).

## Identification

Start-up (`storage::Storage::open` with the board's `SETTINGS`, `DEVICE_INFO` and `LAYOUT_VARIANT` from `boards/<board>/src/storage.rs`) also reads the flash ids (`SpiNor::identification`, `SpiNor::unique_id`, into `storage::NorIdentity`, absent when the chip does not answer), and the board reads the 96-bit unique id of the microcontroller (`hal::flash::unique_id`: `0x1fff_7a10` on the GD32F425, `0x1fff_f7e8` on the GD32F303). The stock identification responder (`protocol::StockIdentity`) answers them for feature command 3 selectors 1 and 3 (the flash ids as manufacturer, device and unique id, zero when absent), and the physical layout as byte 5 of the device settings (command 19).

## Power loss

The host tests (`crates/openwoot/tests/it/storage/`) run the store on a NOR model that cuts the power at every program word and every eighth of a sector erase: a cut word is half programmed, a cut erase leaves the sector erased from its start up to the cut. For 150 fill levels of a four-sector partition, a cut at every step of the next store keeps every other setting, leaves the stored key at its old or its new value and opens the store again without formatting; a cut at any step of a factory reset leaves a working store. Wear over 3000 stores spreads within one erase per sector, and the device-information sector is never erased.

Not covered: a NOR part that leaves an interrupted erase with random or zero content can lose unrelated settings during the repair after the next start (sequential-storage issue 149, mitigation not released for 8.0.2). The consequence is settings falling back to defaults, never a brick or a lost device-information record.
