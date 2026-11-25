use crate::gpio::{Line, Pin};

pub(super) const ALTERNATE_FUNCTION: u8 = 4;

pub trait ClockPin {
    fn into_line(self) -> Line;
}

pub trait DataPin {
    fn into_line(self) -> Line;
}

macro_rules! i2c_pins {
    ($pin_trait:ident: $(($port:literal, $index:literal)),+) => {
        $(
            impl<Mode> $pin_trait for Pin<$port, $index, Mode> {
                fn into_line(self) -> Line {
                    Line::new($port, $index)
                }
            }
        )+
    };
}

i2c_pins!(ClockPin: ('B', 6), ('B', 8));
i2c_pins!(DataPin: ('B', 7), ('B', 9));
