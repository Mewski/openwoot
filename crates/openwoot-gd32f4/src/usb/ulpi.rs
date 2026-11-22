use crate::gpio::{Alternate, OutputType, Pin, Pull, Speed, Unconfigured};

pub(super) const ULPI_TURNAROUND: u8 = 9;

const ULPI_FUNCTION: u8 = 10;

#[derive(Debug)]
pub struct UlpiPins {
    pub clock: Pin<'A', 5, Unconfigured>,
    pub direction: Pin<'C', 2, Unconfigured>,
    pub next: Pin<'C', 3, Unconfigured>,
    pub stop: Pin<'C', 0, Unconfigured>,
    pub data0: Pin<'A', 3, Unconfigured>,
    pub data1: Pin<'B', 0, Unconfigured>,
    pub data2: Pin<'B', 1, Unconfigured>,
    pub data3: Pin<'B', 10, Unconfigured>,
    pub data4: Pin<'B', 11, Unconfigured>,
    pub data5: Pin<'B', 12, Unconfigured>,
    pub data6: Pin<'B', 13, Unconfigured>,
    pub data7: Pin<'B', 5, Unconfigured>,
}

impl UlpiPins {
    pub(super) fn configure(self) {
        let _ = (
            ulpi(self.clock),
            ulpi(self.direction),
            ulpi(self.next),
            ulpi(self.stop),
            ulpi(self.data0),
            ulpi(self.data1),
            ulpi(self.data2),
            ulpi(self.data3),
            ulpi(self.data4),
            ulpi(self.data5),
            ulpi(self.data6),
            ulpi(self.data7),
        );
    }
}

fn ulpi<const PORT: char, const INDEX: u8>(
    pin: Pin<PORT, INDEX, Unconfigured>,
) -> Pin<PORT, INDEX, Alternate<ULPI_FUNCTION>> {
    pin.into_alternate::<ULPI_FUNCTION>(OutputType::PushPull, Speed::Maximum, Pull::None)
}
