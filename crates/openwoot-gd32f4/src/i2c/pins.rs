use crate::gpio::{OutputType, Pin, Pull, Speed};

pub trait ClockPin {
    fn into_i2c(self);
}

pub trait DataPin {
    fn into_i2c(self);
}

macro_rules! i2c_pins {
    ($pin_trait:ident: $(($port:literal, $index:literal)),+) => {
        $(
            impl<Mode> $pin_trait for Pin<$port, $index, Mode> {
                fn into_i2c(self) {
                    let _ = self.into_alternate::<4>(OutputType::OpenDrain, Speed::High, Pull::None);
                }
            }
        )+
    };
}

i2c_pins!(ClockPin: ('B', 6), ('B', 8));
i2c_pins!(DataPin: ('B', 7), ('B', 9));
