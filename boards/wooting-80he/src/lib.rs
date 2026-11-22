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
    gpio::{Level, OutputType, Port, Speed},
    i2c::I2c,
    pac,
    rcu::{self, AhbDivider, ApbDivider, Clocks, Hertz, Pll, PllSystemDivider, TimerMultiplier},
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
pub use openwoot_gd32f4 as hal;
use openwoot_image::{Descriptor, Region, UpdateMode, UsbIds};

include!(concat!(env!("OUT_DIR"), "/memory_layout.rs"));

pub const NAME: &str = "Wooting 80HE";
pub const MANUFACTURER: &str = "Wooting";
pub const FACE: Face = Face {
    mode: Mode::NoGamepad,
    keyboard: KeyboardFormat::NKeyRollover,
    speed: openwoot::usb::Speed::High,
    gamepad_polling: PollingRate::Every1Millisecond,
};
pub const USB_IDS: UsbIds = UsbIds {
    vendor_id: 0x31E3,
    product_id_base: 0x1400,
};
pub const UPDATE_MODE: Option<UpdateMode> = Some(UpdateMode {
    product_id: 0x140F,
    block_size: 256,
});
pub const BOOTLOADER: Region = Region {
    start: 0x0800_0000,
    end: 0x0800_8000,
};
pub const SERIAL_PAGE: Region = Region {
    start: 0x080F_E000,
    end: 0x080F_F000,
};
pub const KEY_ROWS: usize = 6;
pub const KEY_COLUMNS: usize = 17;
pub const DESCRIPTOR: Descriptor = Descriptor::new(NAME, USB_IDS, IMAGE, UPDATE_MODE);
pub const CLOCK_TREE: rcu::Config = rcu::Config {
    crystal: Hertz::from_mhz(25),
    pll: Pll {
        input_divider: 25,
        multiplier: 400,
        system_divider: PllSystemDivider::By2,
        peripheral_divider: 9,
    },
    ahb_divider: AhbDivider::By1,
    apb1_divider: ApbDivider::By4,
    apb2_divider: ApbDivider::By2,
    timer_multiplier: TimerMultiplier::Quadruple,
};

hal::bind_interrupts!(struct Irqs {
    USBHS => usb::InterruptHandler;
    DMA1_Channel2 => hal::spi::InterruptHandler;
    I2C0_EV => hal::i2c::InterruptHandler;
    I2C0_ER => hal::i2c::InterruptHandler;
    DMA0_Channel6 => hal::i2c::InterruptHandler;
    ADC => hal::scan::InterruptHandler<scan::ScanInterrupt>;
});

pub type UsbDriver = usb::Driver;

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
                adc_common: peripherals.adc_common,
                timer: peripherals.timer1,
            },
            &scan::WIRING,
            &clocks,
            Irqs,
        );
        if !scan::SCAN.install(scan::Scanner::new(
            converter,
            scan::ANSI,
            scan::PARALLEL_ORDER,
        )) {
            hal::system::reset();
        }
        let port_a = Port::<'A'>::new(peripherals.gpioa);
        let port_b = Port::<'B'>::new(peripherals.gpiob);
        let port_c = Port::<'C'>::new(peripherals.gpioc);
        let port_d = Port::<'D'>::new(peripherals.gpiod);
        let mut bootloader = Bootloader::new(peripherals.pmu, peripherals.rtc);
        let chip_select = port_d
            .pin2
            .into_output(OutputType::PushPull, Speed::High, Level::High);
        let flash_bus = Spi::new(
            peripherals.spi0,
            port_b.pin3,
            port_a.pin7,
            port_b.pin4,
            Irqs,
        );
        let nor = SpiNor::new(
            Device::new(flash_bus, chip_select, clocks.system),
            storage::NOR_CAPACITY,
        );
        let mut storage = block_on(storage::Storage::open(
            nor,
            storage::SETTINGS,
            storage::DEVICE_INFO,
            storage::LAYOUT_VARIANT,
            bootloader.take_format_request(),
        ));
        let face = block_on(openwoot::profile::stored_face(
            storage.settings.as_mut(),
            FACE,
        ));
        let usb = usb::Driver::new(
            usb::Core {
                global: peripherals.hs_global,
                host: peripherals.hs_host,
                device: peripherals.hs_device,
                power_clock: peripherals.hs_pwrclk,
                fifo: peripherals.hs_fifo,
            },
            usb::UlpiPins {
                clock: port_a.pin5,
                direction: port_c.pin2,
                next: port_c.pin3,
                stop: port_c.pin0,
                data0: port_a.pin3,
                data1: port_b.pin0,
                data2: port_b.pin1,
                data3: port_b.pin10,
                data4: port_b.pin11,
                data5: port_b.pin12,
                data6: port_b.pin13,
                data7: port_b.pin5,
            },
            port_c
                .pin10
                .into_output(OutputType::PushPull, Speed::High, Level::Low),
            Irqs,
            match face.speed {
                openwoot::usb::Speed::High => usb::SpeedLimit::High,
                openwoot::usb::Speed::Full => usb::SpeedLimit::Full,
            },
        );
        let led_bus = I2c::new(peripherals.i2c0, port_b.pin6, port_b.pin7, Irqs, &clocks);
        let led_enable = port_b
            .pin8
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_keeps_clear_of_the_bootloader_and_the_serial_page() {
        assert_eq!(IMAGE.start, BOOTLOADER.end);
        assert_eq!(IMAGE.end, SERIAL_PAGE.start);
    }

    #[test]
    fn the_clock_tree_runs_the_core_at_200_megahertz_and_the_timers_at_1_megahertz_ticks() {
        let clocks = CLOCK_TREE.clocks();
        assert_eq!(clocks.system, Hertz::from_mhz(200));
        assert_eq!(clocks.apb1, Hertz::from_mhz(50));
        assert_eq!(clocks.apb2, Hertz::from_mhz(100));
        assert!(clocks.pll_input >= Hertz::from_mhz(1) && clocks.pll_input <= Hertz::from_mhz(2));
        assert!(clocks.pll_vco >= Hertz::from_mhz(100) && clocks.pll_vco <= Hertz::from_mhz(500));
        assert_eq!(hal::time::prescaler(clocks.apb1_timers), 199);
    }
}
