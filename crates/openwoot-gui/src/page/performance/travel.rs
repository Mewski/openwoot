use std::time::{Duration, Instant};

use gpui_kit::{App, AppContext as _, Context, Entity, Global};
use openwoot_protocol::{
    configuration::Rejection,
    endpoints::GetSensorValues,
    sensors::{self, Reading},
};

use super::visibility::Visibility;
use crate::{device::Failure, store::Store};

const POLL_INTERVAL: Duration = Duration::from_millis(33);
const RETRY_INTERVAL: Duration = Duration::from_millis(500);
const PRESSED_TRAVEL: u16 = 40;

pub(crate) struct TravelFeed {
    store: Entity<Store>,
    values: sensors::Values,
    pressed: Option<usize>,
    unsupported: bool,
    visibility: Visibility,
    polling: bool,
}

struct Shared(Entity<TravelFeed>);

impl Global for Shared {}

impl TravelFeed {
    pub(crate) fn shared(store: &Entity<Store>, cx: &mut App) -> Entity<Self> {
        if let Some(shared) = cx.try_global::<Shared>() {
            return shared.0.clone();
        }
        let store = store.clone();
        let feed = cx.new(|_| Self {
            store,
            values: sensors::Values::new(),
            pressed: None,
            unsupported: false,
            visibility: Visibility::new(Instant::now()),
            polling: false,
        });
        cx.set_global(Shared(feed.clone()));
        feed
    }

    pub(crate) fn travel(&self, key: usize) -> Option<u16> {
        self.values.get(key).copied().flatten()
    }

    pub(crate) fn pressed(&self) -> Option<(usize, u16)> {
        self.pressed.map(|key| (key, self.travel(key).unwrap_or(0)))
    }

    pub(crate) const fn is_unsupported(&self) -> bool {
        self.unsupported
    }

    pub(crate) fn keep_alive(&mut self, cx: &mut Context<Self>) {
        if self.polling {
            self.visibility.rendered(Instant::now());
            return;
        }
        self.polling = true;
        self.visibility.restart(Instant::now());
        let device = self.store.read(cx).device();
        cx.spawn(async move |this, cx| {
            loop {
                let started = Instant::now();
                let shown = this
                    .update(cx, |feed, _| feed.visibility.is_shown(started))
                    .unwrap_or(false);
                if !shown {
                    this.update(cx, |feed, _| feed.stop()).ok();
                    return;
                }
                let reading = device
                    .configure::<GetSensorValues, sensors::Values>(Reading::Travel)
                    .await;
                let pause = if reading.is_ok() {
                    POLL_INTERVAL.saturating_sub(started.elapsed())
                } else {
                    RETRY_INTERVAL
                };
                if this.update(cx, |feed, cx| feed.show(reading, cx)).is_err() {
                    return;
                }
                cx.background_executor().timer(pause).await;
            }
        })
        .detach();
    }

    fn stop(&mut self) {
        self.polling = false;
        self.values.clear();
        self.pressed = None;
    }

    fn show(&mut self, reading: Result<sensors::Values, Failure>, cx: &mut Context<Self>) {
        let mut changed = false;
        let unsupported = matches!(reading, Err(Failure::Rejected(Rejection::Unsupported)));
        if unsupported != self.unsupported {
            self.unsupported = unsupported;
            changed = true;
        }
        if let Ok(values) = reading
            && values != self.values
        {
            if let Some(key) = deepest(&values) {
                self.pressed = Some(key);
            }
            self.values = values;
            changed = true;
        }
        let now = Instant::now();
        if changed || self.visibility.redraw_due(now) {
            self.visibility.notified(now);
            cx.notify();
        }
    }
}

fn deepest(values: &sensors::Values) -> Option<usize> {
    values
        .iter()
        .enumerate()
        .filter_map(|(key, travel)| {
            travel
                .filter(|travel| *travel >= PRESSED_TRAVEL)
                .map(|travel| (key, travel))
        })
        .max_by_key(|(_, travel)| *travel)
        .map(|(key, _)| key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_deepest_pressed_key_is_the_one_shown() {
        let values: sensors::Values = [Some(10), None, Some(900), Some(300)].into_iter().collect();
        assert_eq!(deepest(&values), Some(2));
        let resting: sensors::Values = [Some(10), Some(39)].into_iter().collect();
        assert_eq!(deepest(&resting), None);
    }
}
