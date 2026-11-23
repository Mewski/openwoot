use embassy_futures::select::{Either, select};
use embedded_hal_async::delay::DelayNs;

const PEER_TIMEOUT_MS: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Untimed {}

impl DelayNs for Untimed {
    async fn delay_ns(&mut self, _: u32) {
        match *self {}
    }
}

pub(super) async fn bounded<T>(
    delay: Option<&mut impl DelayNs>,
    work: impl Future<Output = T>,
) -> Option<T> {
    let Some(delay) = delay else {
        return Some(work.await);
    };
    match select(work, delay.delay_ms(PEER_TIMEOUT_MS)).await {
        Either::First(value) => Some(value),
        Either::Second(()) => None,
    }
}
