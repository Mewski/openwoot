#![no_std]
#![forbid(unsafe_code)]

pub use cortex_m_rt::entry;
use hal::{
    gpio::{Level, Output, OutputType, Pin, Port, Speed},
    pac,
    rcu::{self, AhbDivider, ApbDivider, Clocks, PllMultiplier},
    system::Bootloader,
    usb,
};
use openwoot::Keyboard;
pub use openwoot_gd32f3 as hal;
use openwoot_image::{Descriptor, Region, UpdateMode, UsbIds};

include!(concat!(env!("OUT_DIR"), "/memory_layout.rs"));

pub const NAME: &str = "Wooting 60HE+";
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
pub const DESCRIPTOR: Descriptor = Descriptor::new(NAME, USB_IDS, IMAGE, UPDATE_MODE);
pub const CLOCK_TREE: rcu::Config = rcu::Config {
    pll_multiplier: PllMultiplier::new::<30>(),
    ahb_divider: AhbDivider::By1,
    apb1_divider: ApbDivider::By2,
    apb2_divider: ApbDivider::By1,
};

hal::bind_interrupts!(struct Irqs {
    USBD_HP_CAN0_TX => usb::InterruptHandler;
    USBD_LP_CAN0_RX0 => usb::InterruptHandler;
});

#[derive(Debug)]
pub struct Board {
    pub clocks: Clocks,
    pub bootloader: Bootloader,
    pub usb: usb::Driver<Pin<'A', 15, Output>>,
}

impl Board {
    #[must_use]
    pub fn new(peripherals: pac::Peripherals) -> Self {
        let clocks = CLOCK_TREE.configure(peripherals.rcu, &peripherals.pmu);
        hal::time::start(peripherals.timer4, &clocks);
        let port_a = Port::<'A'>::new(peripherals.gpioa);
        let usb = usb::Driver::new(
            peripherals.usbd,
            peripherals.usbd_ram,
            &peripherals.afio,
            port_a
                .pin15
                .into_output(OutputType::PushPull, Speed::High, Level::Low),
            Irqs,
            &clocks,
        );
        Self {
            clocks,
            bootloader: Bootloader::new(peripherals.pmu, peripherals.bkp),
            usb,
        }
    }
}

impl Keyboard for Board {
    const DESCRIPTOR: Descriptor = DESCRIPTOR;
    const KEY_COLUMNS: usize = KEY_COLUMNS;
    const KEY_ROWS: usize = KEY_ROWS;
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
    fn the_clock_tree_runs_the_core_at_120_megahertz_and_the_timers_at_1_megahertz_ticks() {
        let clocks = CLOCK_TREE.clocks();
        assert_eq!(clocks.system, rcu::Hertz::from_mhz(120));
        assert_eq!(clocks.apb1, rcu::Hertz::from_mhz(60));
        assert_eq!(clocks.apb2, rcu::Hertz::from_mhz(120));
        assert_eq!(hal::time::prescaler(clocks.apb1_timers), 119);
    }
}
