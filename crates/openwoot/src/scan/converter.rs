#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sample {
    pub reading: u16,
    pub reference: u16,
}

pub trait Converter {
    fn acknowledge();

    fn request_interrupt();

    fn sample(&self, rank: usize) -> Sample;

    fn select_address(&mut self, address: u8);

    fn select_input(&mut self, previous: u8, next: u8);

    fn enter_single(&mut self, previous: u8);

    fn enter_parallel(&mut self, order: &[u8], address: u8);

    fn sweep_enter_parallel(&mut self, order: Option<&[u8]>);

    fn trigger_after(&mut self, delay_us: u16);

    fn retrigger_after(&mut self, delay_us: u16);

    fn trigger_at(&mut self, ticks: u16);

    fn end_cycle(&mut self, period_us: u16);

    fn recover(&mut self) {}

    fn micros(&self) -> u16;
}
