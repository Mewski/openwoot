use super::{
    InputReport,
    descriptor::{Builder, Collection, MainFlags},
    usage::{desktop, page, simulation, unit},
};

pub const DESCRIPTOR: &[u8] = &descriptor().to_array::<{ descriptor().len() }>();
pub(super) const BUTTONS: u8 = 12;
pub const STICK_MAX: u16 = 0x7FFE;
pub const STICK_CENTER: u16 = STICK_MAX / 2;
pub const SLIDER_MAX: u16 = 0x7FFF;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Hat {
    #[default]
    Centered,
    Up,
    UpRight,
    Right,
    DownRight,
    Down,
    DownLeft,
    Left,
    UpLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub buttons: u16,
    pub x: u16,
    pub y: u16,
    pub z: u16,
    pub rz: u16,
    pub rx: u16,
    pub ry: u16,
    pub throttle: u16,
    pub elevator: u16,
    pub accelerator: u16,
    pub rudder: u16,
    pub trigger: u16,
    pub hat: Hat,
}

impl Hat {
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::Centered => 0,
            Self::Up => 1,
            Self::UpRight => 2,
            Self::Right => 3,
            Self::DownRight => 4,
            Self::Down => 5,
            Self::DownLeft => 6,
            Self::Left => 7,
            Self::UpLeft => 8,
        }
    }
}

impl Report {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buttons: 0,
            x: STICK_CENTER,
            y: STICK_CENTER,
            z: STICK_CENTER,
            rz: STICK_CENTER,
            rx: STICK_CENTER,
            ry: STICK_CENTER,
            throttle: 0,
            elevator: 0,
            accelerator: 0,
            rudder: 0,
            trigger: 0,
            hat: Hat::Centered,
        }
    }
}

impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}

impl InputReport for Report {
    type Bytes = [u8; 25];

    const ID: Option<u8> = None;
    const LEN: usize = 25;

    fn to_bytes(&self) -> Self::Bytes {
        let mut bytes = [0; 25];
        let words = [
            self.buttons & ((1 << BUTTONS) - 1),
            self.x.min(STICK_MAX),
            self.y.min(STICK_MAX),
            self.z.min(STICK_MAX),
            self.rz.min(STICK_MAX),
            self.rx.min(STICK_MAX),
            self.ry.min(STICK_MAX),
            self.throttle.min(SLIDER_MAX),
            self.elevator.min(SLIDER_MAX),
            self.accelerator.min(SLIDER_MAX),
            self.rudder.min(SLIDER_MAX),
            self.trigger.min(SLIDER_MAX),
        ];
        for (chunk, word) in bytes.as_chunks_mut::<2>().0.iter_mut().zip(words) {
            *chunk = word.to_le_bytes();
        }
        if let Some(hat) = bytes.last_mut() {
            *hat = self.hat.value();
        }
        bytes
    }
}

const fn descriptor() -> Builder {
    Builder::new()
        .usage_page(page::GENERIC_DESKTOP)
        .usage(desktop::GAMEPAD)
        .collection(Collection::Application)
        .usage_page(page::BUTTON)
        .usage_minimum(1)
        .usage_maximum(BUTTONS as u16)
        .logical_minimum(0)
        .logical_maximum(1)
        .report_count(BUTTONS as usize)
        .report_size(1)
        .input(MainFlags::DATA.variable())
        .report_size((16 - BUTTONS) as usize)
        .report_count(1)
        .input(MainFlags::CONSTANT.variable())
        .usage_page(page::GENERIC_DESKTOP)
        .collection(Collection::Physical)
        .usage(desktop::X_AXIS)
        .usage(desktop::Y_AXIS)
        .logical_minimum(0)
        .logical_maximum(STICK_MAX as i32)
        .physical_minimum(0)
        .physical_maximum(STICK_MAX as i32)
        .unit(unit::NONE)
        .report_size(16)
        .report_count(2)
        .input(MainFlags::DATA.variable())
        .end_collection()
        .collection(Collection::Physical)
        .usage(desktop::Z_AXIS)
        .usage(desktop::RZ)
        .input(MainFlags::DATA.variable())
        .end_collection()
        .usage(desktop::RX)
        .usage(desktop::RY)
        .input(MainFlags::DATA.variable())
        .usage_page(page::SIMULATION)
        .usage(simulation::THROTTLE)
        .usage(simulation::ELEVATOR)
        .usage(simulation::ACCELERATOR)
        .usage(simulation::RUDDER)
        .usage(simulation::TRIGGER)
        .logical_minimum(0)
        .logical_maximum(SLIDER_MAX as i32)
        .physical_minimum(0)
        .physical_maximum(SLIDER_MAX as i32)
        .unit(unit::NONE)
        .report_size(16)
        .report_count(5)
        .input(MainFlags::DATA.variable())
        .usage_page(page::GENERIC_DESKTOP)
        .usage(desktop::HAT_SWITCH)
        .logical_minimum(1)
        .logical_maximum(8)
        .physical_minimum(0)
        .physical_maximum(315)
        .unit(unit::DEGREES)
        .report_size(8)
        .report_count(1)
        .input(MainFlags::DATA.variable().null_state())
        .end_collection()
}
