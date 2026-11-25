use crate::gpio::{Line, Pin};

pub trait ClockPin {
    fn into_line(self) -> Line;
}

pub trait DataPin {
    fn into_line(self) -> Line;
}

impl<Mode> ClockPin for Pin<'B', 6, Mode> {
    fn into_line(self) -> Line {
        Line::new('B', 6)
    }
}

impl<Mode> DataPin for Pin<'B', 7, Mode> {
    fn into_line(self) -> Line {
        Line::new('B', 7)
    }
}
