use crate::gpio::{OutputType, Pin, Pull, Speed};

pub trait ClockPin {
    fn into_spi(self);
}

pub trait DataOutPin {
    fn into_spi(self);
}

pub trait DataInPin {
    fn into_spi(self);
}

impl<Mode> ClockPin for Pin<'B', 13, Mode> {
    fn into_spi(self) {
        let _ = self.into_alternate(OutputType::PushPull, Speed::High);
    }
}

impl<Mode> DataOutPin for Pin<'B', 15, Mode> {
    fn into_spi(self) {
        let _ = self.into_alternate(OutputType::PushPull, Speed::High);
    }
}

impl<Mode> DataInPin for Pin<'B', 14, Mode> {
    fn into_spi(self) {
        let _ = self.into_input(Pull::None);
    }
}
