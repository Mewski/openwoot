use embassy_futures::{
    block_on,
    select::{Either, select},
    yield_now,
};
use openwoot::runtime::LedWindow;

use super::hold_clock;

pub(crate) fn between_scans<T>(window: &LedWindow, lighting: impl Future<Output = T>) -> T {
    let _clock = hold_clock();
    let scanning = async {
        loop {
            window.grant().await;
            yield_now().await;
        }
    };
    match block_on(select(lighting, scanning)) {
        Either::First(output) => output,
        Either::Second(never) => never,
    }
}
