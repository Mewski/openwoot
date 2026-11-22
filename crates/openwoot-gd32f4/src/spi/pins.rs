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

macro_rules! spi_pins {
    ($pin_trait:ident: $(($port:literal, $index:literal)),+) => {
        $(
            impl<Mode> $pin_trait for Pin<$port, $index, Mode> {
                fn into_spi(self) {
                    let _ = self.into_alternate::<5>(OutputType::PushPull, Speed::High, Pull::None);
                }
            }
        )+
    };
}

spi_pins!(ClockPin: ('A', 5), ('B', 3));
spi_pins!(DataOutPin: ('A', 7), ('B', 5));
spi_pins!(DataInPin: ('A', 6), ('B', 4));
