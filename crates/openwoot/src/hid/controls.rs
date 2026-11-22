use super::{
    InputReport,
    descriptor::{Builder, Collection, MainFlags},
    usage::{consumer, desktop, page},
};

pub const DESCRIPTOR: &[u8] = &descriptor().to_array::<{ descriptor().len() }>();
pub const SYSTEM_REPORT_ID: u8 = 1;
pub const CONSUMER_REPORT_ID: u8 = 2;
pub const MOUSE_REPORT_ID: u8 = 3;
pub const CONSUMER_SLOTS: usize = 3;
pub const MOUSE_BUTTONS: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemControl {
    PowerDown,
    Sleep,
    WakeUp,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystemReport {
    pub control: Option<SystemControl>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConsumerReport {
    pub usages: [u16; CONSUMER_SLOTS],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MouseReport {
    pub buttons: u8,
    pub x: i8,
    pub y: i8,
}

impl SystemControl {
    #[must_use]
    pub const fn usage(self) -> u16 {
        desktop::SYSTEM_POWER_DOWN + self.index() as u16 - 1
    }

    const fn index(self) -> u8 {
        match self {
            Self::PowerDown => 1,
            Self::Sleep => 2,
            Self::WakeUp => 3,
        }
    }
}

impl InputReport for SystemReport {
    type Bytes = [u8; 2];

    const ID: Option<u8> = Some(SYSTEM_REPORT_ID);
    const LEN: usize = 1;

    fn to_bytes(&self) -> Self::Bytes {
        [
            SYSTEM_REPORT_ID,
            self.control.map_or(0, SystemControl::index),
        ]
    }
}

impl InputReport for ConsumerReport {
    type Bytes = [u8; 7];

    const ID: Option<u8> = Some(CONSUMER_REPORT_ID);
    const LEN: usize = 6;

    fn to_bytes(&self) -> Self::Bytes {
        let [
            [first_low, first_high],
            [second_low, second_high],
            [third_low, third_high],
        ] = self.usages.map(u16::to_le_bytes);
        [
            CONSUMER_REPORT_ID,
            first_low,
            first_high,
            second_low,
            second_high,
            third_low,
            third_high,
        ]
    }
}

impl InputReport for MouseReport {
    type Bytes = [u8; 4];

    const ID: Option<u8> = Some(MOUSE_REPORT_ID);
    const LEN: usize = 3;

    fn to_bytes(&self) -> Self::Bytes {
        let [x] = self.x.to_le_bytes();
        let [y] = self.y.to_le_bytes();
        [
            MOUSE_REPORT_ID,
            self.buttons & ((1 << MOUSE_BUTTONS) - 1),
            x,
            y,
        ]
    }
}

const fn descriptor() -> Builder {
    Builder::new()
        .usage_page(page::GENERIC_DESKTOP)
        .usage(desktop::SYSTEM_CONTROL)
        .collection(Collection::Application)
        .report_id(SYSTEM_REPORT_ID)
        .usage_minimum(desktop::SYSTEM_POWER_DOWN)
        .usage_maximum(desktop::SYSTEM_WAKE_UP)
        .logical_minimum(1)
        .logical_maximum(3)
        .report_count(1)
        .report_size(8)
        .input(MainFlags::DATA)
        .end_collection()
        .usage_page(page::CONSUMER)
        .usage(consumer::CONSUMER_CONTROL)
        .collection(Collection::Application)
        .report_id(CONSUMER_REPORT_ID)
        .usage_minimum(1)
        .usage_maximum(consumer::LAST)
        .logical_minimum(1)
        .logical_maximum(consumer::LAST as i32)
        .report_count(CONSUMER_SLOTS)
        .report_size(16)
        .input(MainFlags::DATA)
        .end_collection()
        .usage_page(page::GENERIC_DESKTOP)
        .usage(desktop::MOUSE)
        .collection(Collection::Application)
        .report_id(MOUSE_REPORT_ID)
        .usage(desktop::POINTER)
        .collection(Collection::Physical)
        .usage_page(page::BUTTON)
        .usage_minimum(1)
        .usage_maximum(MOUSE_BUTTONS as u16)
        .logical_minimum(0)
        .logical_maximum(1)
        .report_size(1)
        .report_count(MOUSE_BUTTONS as usize)
        .input(MainFlags::DATA.variable())
        .report_count((8 - MOUSE_BUTTONS) as usize)
        .input(MainFlags::CONSTANT.variable())
        .usage_page(page::GENERIC_DESKTOP)
        .usage(desktop::X_AXIS)
        .usage(desktop::Y_AXIS)
        .logical_minimum(-127)
        .logical_maximum(127)
        .report_size(8)
        .report_count(2)
        .input(MainFlags::DATA.variable().relative())
        .end_collection()
        .end_collection()
}
