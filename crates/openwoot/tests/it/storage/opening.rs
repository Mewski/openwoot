use core::future::{Future, ready};

use embassy_futures::block_on;
use embedded_hal::spi::{Error as SpiError, ErrorKind, ErrorType};
use embedded_hal_async::spi::{Operation, SpiDevice};
use openwoot::storage::{Error, InDeviceInfo, NorLayout, NorSector, Settings, Storage};
use openwoot_drivers::spi_nor::SpiNor;
use openwoot_image::Region;

const CAPACITY: u32 = 0x80_0000;
const DEVICE_INFO: Region = Region {
    start: 0x7F_F000,
    end: CAPACITY,
};
const LAYOUT: NorLayout = NorLayout {
    capacity: CAPACITY,
    settings: Region {
        start: 0,
        end: 0x8_0000,
    },
    device_info: Some(DEVICE_INFO),
};

#[derive(Debug)]
struct Unplugged;

#[derive(Debug)]
struct StuckHigh;

#[derive(Debug)]
struct NoAnswer;

impl SpiError for NoAnswer {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

impl ErrorType for Unplugged {
    type Error = NoAnswer;
}

impl SpiDevice for Unplugged {
    fn transaction(
        &mut self,
        _operations: &mut [Operation<'_, u8>],
    ) -> impl Future<Output = Result<(), NoAnswer>> {
        ready(Err(NoAnswer))
    }
}

impl ErrorType for StuckHigh {
    type Error = NoAnswer;
}

impl SpiDevice for StuckHigh {
    fn transaction(
        &mut self,
        operations: &mut [Operation<'_, u8>],
    ) -> impl Future<Output = Result<(), NoAnswer>> {
        for operation in operations {
            match operation {
                Operation::Read(bytes)
                | Operation::Transfer(bytes, _)
                | Operation::TransferInPlace(bytes) => bytes.fill(0xFF),
                Operation::Write(_) | Operation::DelayNs(_) => {}
            }
        }
        ready(Ok(()))
    }
}

fn open_on<D: SpiDevice>(device: D, cell: &mut Settings<SpiNor<D>>) -> Storage<'_, SpiNor<D>> {
    block_on(Storage::open(
        SpiNor::new(device, CAPACITY),
        cell,
        LAYOUT.settings_region(),
        NorSector {
            offset: DEVICE_INFO.start,
        },
        InDeviceInfo,
        false,
    ))
}

#[test]
fn a_flash_that_cannot_be_read_leaves_the_keyboard_without_stored_settings() {
    let mut cell = Settings::new();
    let storage = open_on(Unplugged, &mut cell);
    assert!(storage.settings.is_none());
    assert!(
        matches!(storage.opened, Err(Error::Flash(_))),
        "{:?}",
        storage.opened
    );
    assert_eq!(storage.device_info, None);
    assert_eq!(storage.nor_identity, None);
    assert_eq!(storage.layout_variant, 0);
}

#[test]
fn a_flash_that_always_reads_busy_lets_the_start_up_finish_without_stored_settings() {
    let mut cell = Settings::new();
    let storage = open_on(StuckHigh, &mut cell);
    assert!(storage.settings.is_none());
    assert!(
        matches!(storage.opened, Err(Error::Flash(_))),
        "{:?}",
        storage.opened
    );
    assert_eq!(storage.device_info, None);
    assert_eq!(storage.nor_identity, None);
    assert_eq!(storage.layout_variant, 0);
}
