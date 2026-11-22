#![no_std]
#![forbid(unsafe_code)]

pub use cortex_m_rt::entry;
use hal::{
    gpio::{Level, OutputType, Port, Speed},
    pac,
    rcu::{self, AhbDivider, ApbDivider, Clocks, Hertz, Pll, PllSystemDivider, TimerMultiplier},
    system::Bootloader,
    usb,
};
use openwoot::Keyboard;
pub use openwoot_gd32f4 as hal;
use openwoot_image::{Descriptor, Region, UpdateMode, UsbIds};

include!(concat!(env!("OUT_DIR"), "/memory_layout.rs"));

pub const NAME: &str = "Wooting 80HE";
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
});

#[derive(Debug)]
pub struct Board {
    pub clocks: Clocks,
    pub bootloader: Bootloader,
    pub usb: usb::Driver,
}

impl Board {
    #[must_use]
    pub fn new(peripherals: pac::Peripherals) -> Self {
        let clocks = CLOCK_TREE.configure(peripherals.rcu, &peripherals.pmu);
        hal::time::start(peripherals.timer4, &clocks);
        let port_a = Port::<'A'>::new(peripherals.gpioa);
        let port_b = Port::<'B'>::new(peripherals.gpiob);
        let port_c = Port::<'C'>::new(peripherals.gpioc);
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
            usb::SpeedLimit::High,
        );
        Self {
            clocks,
            bootloader: Bootloader::new(peripherals.pmu, peripherals.rtc),
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
