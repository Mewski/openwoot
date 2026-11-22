use crate::gpio::{OutputType, Pin, Speed};

pub trait ClockPin {
    fn into_i2c(self);
}

pub trait DataPin {
    fn into_i2c(self);
}

impl<Mode> ClockPin for Pin<'B', 6, Mode> {
    fn into_i2c(self) {
        let _ = self.into_alternate(OutputType::OpenDrain, Speed::High);
    }
}

impl<Mode> DataPin for Pin<'B', 7, Mode> {
    fn into_i2c(self) {
        let _ = self.into_alternate(OutputType::OpenDrain, Speed::High);
    }
}
