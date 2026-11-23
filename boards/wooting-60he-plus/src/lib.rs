#![no_std]
#![forbid(unsafe_code)]

pub mod keys;
pub mod lighting;
pub mod profiles;
pub mod scan;
pub mod storage;

pub use cortex_m_rt::entry;
use embassy_futures::block_on;
use hal::{
    gpio::{Level, Output, OutputType, Pin, Port, Speed},
    i2c::I2c,
    pac,
    rcu::{self, AhbDivider, ApbDivider, Clocks, PllMultiplier},
    spi::{Device, Spi},
    system::Bootloader,
    usb,
};
use openwoot::{
    lighting::Matrix,
    protocol::StockIdentity,
    usb::{Face, Identity, KeyboardFormat, Mode, PollingRate, serial_number_from_page},
};
use openwoot_drivers::{is31fl3741::Is31fl3741, spi_nor::SpiNor};
pub use openwoot_gd32f3 as hal;
use openwoot_image::{Descriptor, Region, UpdateMode, UsbIds};

include!(concat!(env!("OUT_DIR"), "/memory_layout.rs"));

pub const NAME: &str = "Wooting 60HE+";
pub const MANUFACTURER: &str = "Wooting";
pub const FACE: Face = Face {
    mode: Mode::NoGamepad,
    keyboard: KeyboardFormat::NKeyRollover,
    speed: openwoot::usb::Speed::Full,
    gamepad_polling: PollingRate::Every1Millisecond,
};
pub const USB_IDS: UsbIds = UsbIds {
    vendor_id: 0x31E3,
    product_id_base: 0x1320,
};
pub const UPDATE_MODE: Option<UpdateMode> = None;
pub const BOOTLOADER: Region = Region {
    start: 0x0800_0000,
    end: 0x0800_7000,
};
pub const SERIAL_PAGE: Region = Region {
    start: 0x080F_D000,
    end: 0x080F_E000,
};
pub const KEY_ROWS: usize = 6;
pub const KEY_COLUMNS: usize = 14;
pub const DESCRIPTOR: Descriptor = Descriptor::new(NAME, USB_IDS, IMAGE, RAM, UPDATE_MODE);
pub const CLOCK_TREE: rcu::ClockTree = rcu::ClockTree {
    pll_multiplier: PllMultiplier::new::<30>(),
    ahb_divider: AhbDivider::By1,
    apb1_divider: ApbDivider::By2,
    apb2_divider: ApbDivider::By1,
};

const _: () = assert!(
    IMAGE.start == BOOTLOADER.end && IMAGE.end == SERIAL_PAGE.start,
    "the image area of `memory.x` starts behind the bootloader and ends at the serial page"
);

const _: () = assert!(
    CLOCK_TREE.is_valid() && usb::supports_system_clock(CLOCK_TREE.clocks().system),
    "the clock tree stays within the bus limits and gives the USB controller its 48 MHz"
);

hal::bind_interrupts!(struct Irqs {
    USBD_HP_CAN0_TX => usb::InterruptHandler;
    USBD_LP_CAN0_RX0 => usb::InterruptHandler;
    DMA0_Channel3 => hal::spi::InterruptHandler;
    I2C0_EV => hal::i2c::InterruptHandler;
    I2C0_ER => hal::i2c::InterruptHandler;
    DMA0_Channel5 => hal::i2c::InterruptHandler;
    ADC0_1 => hal::scan::InterruptHandler<scan::Interrupt>;
});

pub type UsbDriver = usb::Driver<Pin<'A', 15, Output>>;

#[derive(Debug)]
pub struct Board {
    pub clocks: Clocks,
    pub serial_number: Option<&'static str>,
    pub bootloader: Bootloader,
    pub face: Face,
    pub storage: storage::Storage,
    pub usb: UsbDriver,
    pub leds: lighting::LedDriver,
}

impl Board {
    #[must_use]
    pub fn new(peripherals: pac::Peripherals) -> Self {
        let clocks = CLOCK_TREE.configure(peripherals.rcu, &peripherals.pmu);
        hal::time::start(peripherals.timer4, &clocks);
        let converter = hal::scan::Converter::new(
            hal::scan::Peripherals {
                adc: peripherals.adc0,
                reference_adc: peripherals.adc1,
                timer: peripherals.timer1,
            },
            &scan::WIRING,
            &clocks,
            Irqs,
        );
        install_scanner(converter);
        let port_a = Port::<'A'>::new(peripherals.gpioa);
        let port_b = Port::<'B'>::new(peripherals.gpiob);
        let mut bootloader = Bootloader::new(peripherals.pmu, peripherals.bkp);
        let chip_select = port_b
            .pin12
            .into_output(OutputType::PushPull, Speed::High, Level::High);
        let flash_bus = Spi::new(
            peripherals.spi1,
            port_b.pin13,
            port_b.pin15,
            port_b.pin14,
            Irqs,
        );
        let nor = SpiNor::new(
            Device::new(flash_bus, chip_select, clocks.system),
            storage::NOR_CAPACITY,
        );
        let (storage, face) = open_storage(nor, bootloader.take_format_request());
        let usb = usb::Driver::new(
            usb::Core {
                controller: peripherals.usbd,
                packet_memory: peripherals.usbd_ram,
            },
            &peripherals.afio,
            port_a
                .pin15
                .into_output(OutputType::PushPull, Speed::High, Level::Low),
            Irqs,
            &clocks,
        );
        let led_bus = I2c::new(peripherals.i2c0, port_b.pin6, port_b.pin7, Irqs, &clocks);
        let led_enable = port_b
            .pin5
            .into_output(OutputType::PushPull, Speed::High, Level::Low);
        let leds = Matrix::new(
            Is31fl3741::new(led_bus, led_enable, lighting::LED_DRIVER_ADDRESS),
            lighting::LED_DRIVER_RUNNING,
        );
        scan::select_layout(storage.layout_variant);
        Self {
            clocks,
            serial_number: serial_number(),
            bootloader,
            face,
            storage,
            usb,
            leds,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> Identity<'static> {
        Identity {
            ids: USB_IDS,
            manufacturer: MANUFACTURER,
            product: NAME,
            serial_number: self.serial_number,
        }
    }

    #[must_use]
    pub fn stock_identity(&self) -> StockIdentity {
        StockIdentity {
            serial: self.serial_number,
            face: self.face,
            layout: self.storage.layout_variant,
            mcu_id: hal::flash::unique_id(),
            nor_identity: self.storage.nor_identity,
        }
    }
}

fn serial_number() -> Option<&'static str> {
    hal::flash::read(SERIAL_PAGE).and_then(serial_number_from_page)
}

#[inline(never)]
fn install_scanner(converter: scan::Converter) {
    let installed = scan::SENSORS.try_take().is_some_and(|sensors| {
        scan::SCAN
            .install(|| scan::Scanner::new(converter, scan::ANSI, scan::PARALLEL_ORDER, sensors))
    });
    if !installed {
        hal::system::reset();
    }
}

#[inline(never)]
fn open_storage(nor: storage::Nor, format: bool) -> (storage::Storage, Face) {
    let mut storage = block_on(storage::Storage::open(
        nor,
        storage::SETTINGS_STORE.take(),
        storage::SETTINGS,
        storage::DEVICE_INFO,
        storage::LAYOUT_VARIANT,
        format,
    ));
    let face = block_on(openwoot::profile::stored_face(
        storage.settings.as_deref_mut(),
        FACE,
    ));
    (storage, face)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_keeps_clear_of_the_bootloader_and_the_serial_page() {
        assert_eq!(IMAGE.start, BOOTLOADER.end);
        assert_eq!(IMAGE.end, SERIAL_PAGE.start);
    }

    #[test]
    fn the_descriptor_carries_the_image_area_and_ram_of_memory_x() {
        let descriptor = Descriptor::from_bytes(&DESCRIPTOR.to_bytes()).unwrap();
        assert_eq!(descriptor, DESCRIPTOR);
        assert_eq!(descriptor.image(), IMAGE);
        assert_eq!(
            descriptor.ram(),
            Region {
                start: 0x2000_0000,
                end: 0x2001_8000
            }
        );
        assert_eq!(descriptor.update_mode(), None);
    }

    #[test]
    fn the_clock_tree_runs_the_core_at_120_megahertz_and_the_timers_at_1_megahertz_ticks() {
        let clocks = CLOCK_TREE.clocks();
        assert_eq!(clocks.system, rcu::Hertz::from_mhz(120));
        assert_eq!(clocks.apb1, rcu::Hertz::from_mhz(60));
        assert_eq!(clocks.apb2, rcu::Hertz::from_mhz(120));
        assert_eq!(hal::time::prescaler(clocks.apb1_timers), 119);
    }
}
