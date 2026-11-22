use super::{
    Pin, Unconfigured,
    registers::{enable_clock, port_index},
};
use crate::pac;

#[derive(Debug)]
pub struct Port<const PORT: char> {
    pub pin0: Pin<PORT, 0, Unconfigured>,
    pub pin1: Pin<PORT, 1, Unconfigured>,
    pub pin2: Pin<PORT, 2, Unconfigured>,
    pub pin3: Pin<PORT, 3, Unconfigured>,
    pub pin4: Pin<PORT, 4, Unconfigured>,
    pub pin5: Pin<PORT, 5, Unconfigured>,
    pub pin6: Pin<PORT, 6, Unconfigured>,
    pub pin7: Pin<PORT, 7, Unconfigured>,
    pub pin8: Pin<PORT, 8, Unconfigured>,
    pub pin9: Pin<PORT, 9, Unconfigured>,
    pub pin10: Pin<PORT, 10, Unconfigured>,
    pub pin11: Pin<PORT, 11, Unconfigured>,
    pub pin12: Pin<PORT, 12, Unconfigured>,
    pub pin13: Pin<PORT, 13, Unconfigured>,
    pub pin14: Pin<PORT, 14, Unconfigured>,
    pub pin15: Pin<PORT, 15, Unconfigured>,
}

impl<const PORT: char> Port<PORT> {
    const fn split() -> Self {
        Self {
            pin0: Pin::new(),
            pin1: Pin::new(),
            pin2: Pin::new(),
            pin3: Pin::new(),
            pin4: Pin::new(),
            pin5: Pin::new(),
            pin6: Pin::new(),
            pin7: Pin::new(),
            pin8: Pin::new(),
            pin9: Pin::new(),
            pin10: Pin::new(),
            pin11: Pin::new(),
            pin12: Pin::new(),
            pin13: Pin::new(),
            pin14: Pin::new(),
            pin15: Pin::new(),
        }
    }
}

macro_rules! ports {
    ($($port:ident: $letter:literal;)*) => {
        $(
            impl Port<$letter> {
                #[must_use]
                pub fn new(port: pac::$port) -> Self {
                    let _ = port;
                    enable_clock(port_index($letter));
                    Self::split()
                }
            }
        )*
    };
}

ports! {
    Gpioa: 'A';
    Gpiob: 'B';
    Gpioc: 'C';
    Gpiod: 'D';
    Gpioe: 'E';
}
