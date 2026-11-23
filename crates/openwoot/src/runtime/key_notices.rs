use core::cell::Cell;

use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
};

use crate::{
    keys::{NOTICE_CAPACITY, Notice},
    lighting::{BLACK, Lighting, Overlay, OverlayId, Span},
};

const NOTICES_BESIDE_A_KEY_PASS: usize = 8;
const KEY_NOTICE_CAPACITY: usize = NOTICE_CAPACITY + NOTICES_BESIDE_A_KEY_PASS;

const BRIGHTNESS_REPEAT_MS: u32 = 50;
const PROFILE_BLINK_MS: u16 = 1000;
const PROFILE_BLINK_PERIOD_MS: u16 = 250;
const WHITE: openwoot_protocol::lighting::Rgb = openwoot_protocol::lighting::Rgb {
    red: 255,
    green: 255,
    blue: 255,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Repeat {
    increase: bool,
    due_ms: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BrightnessKeys {
    held: Option<bool>,
    pressed: Option<bool>,
}

pub struct KeyNotices {
    channel: Channel<CriticalSectionRawMutex, Notice, KEY_NOTICE_CAPACITY>,
    brightness: Mutex<CriticalSectionRawMutex, Cell<BrightnessKeys>>,
    repeat: Mutex<CriticalSectionRawMutex, Cell<Option<Repeat>>>,
}

impl BrightnessKeys {
    fn with_key(self, increase: bool, held: bool) -> Self {
        if held {
            Self {
                held: Some(increase),
                pressed: Some(increase),
            }
        } else if self.held == Some(increase) {
            Self { held: None, ..self }
        } else {
            self
        }
    }
}

impl KeyNotices {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            channel: Channel::new(),
            brightness: Mutex::new(Cell::new(BrightnessKeys {
                held: None,
                pressed: None,
            })),
            repeat: Mutex::new(Cell::new(None)),
        }
    }

    pub fn send(&self, notice: Notice) -> bool {
        if let Notice::Brightness { increase, held } = notice {
            self.brightness_key(increase, held);
            return true;
        }
        self.channel.try_send(notice).is_ok()
    }

    pub async fn ready(&self) {
        self.channel.ready_to_receive().await;
    }

    pub fn apply<
        H,
        const ROWS: usize,
        const COLUMNS: usize,
        const PAGES: usize,
        const LENGTH: usize,
    >(
        &self,
        lighting: &mut Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>,
        now_ms: u32,
    ) where
        H: crate::lighting::Lightbar + crate::lighting::Signals,
    {
        while let Ok(notice) = self.channel.try_receive() {
            self.apply_notice(lighting, notice, now_ms);
        }
        let keys = self.brightness.lock(|cell| {
            let keys = cell.get();
            cell.set(BrightnessKeys {
                pressed: None,
                ..keys
            });
            keys
        });
        if let Some(increase) = keys.pressed {
            lighting.step_brightness(increase, now_ms);
        }
        let repeat = match (keys.held, self.repeat.lock(Cell::get)) {
            (None, _) => None,
            (Some(increase), Some(repeat))
                if repeat.increase == increase && keys.pressed.is_none() =>
            {
                Some(repeat)
            }
            (Some(increase), _) => Some(Repeat {
                increase,
                due_ms: now_ms.wrapping_add(BRIGHTNESS_REPEAT_MS),
            }),
        };
        let repeat = repeat.map(|repeat| {
            if now_ms.wrapping_sub(repeat.due_ms).cast_signed() < 0 {
                return repeat;
            }
            lighting.step_brightness(repeat.increase, now_ms);
            Repeat {
                due_ms: repeat.due_ms.wrapping_add(BRIGHTNESS_REPEAT_MS),
                ..repeat
            }
        });
        self.repeat.lock(|cell| cell.set(repeat));
    }

    fn brightness_key(&self, increase: bool, held: bool) {
        self.brightness
            .lock(|cell| cell.set(cell.get().with_key(increase, held)));
    }

    fn apply_notice<
        H,
        const ROWS: usize,
        const COLUMNS: usize,
        const PAGES: usize,
        const LENGTH: usize,
    >(
        &self,
        lighting: &mut Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>,
        notice: Notice,
        now_ms: u32,
    ) where
        H: crate::lighting::Lightbar + crate::lighting::Signals,
    {
        match notice {
            Notice::Key { key, event } => lighting.key_event(key, event, now_ms),
            Notice::ShowLayer(layer) => {
                lighting.show_layer(layer);
                lighting.hooks_mut().layer_changed(layer);
            }
            Notice::Locks(locks) => {
                lighting.update_lock_indicators(locks, now_ms);
                lighting.hooks_mut().locks_changed(locks);
            }
            Notice::Brightness { increase, held } => self.brightness_key(increase, held),
            Notice::Disabled { key, disabled } => {
                if disabled {
                    lighting.start_overlay(
                        OverlayId::Key(key),
                        Span::Forever,
                        Overlay::Solid { key, color: BLACK },
                        now_ms,
                    );
                } else {
                    lighting.stop_overlay(OverlayId::Key(key));
                }
            }
            Notice::Toggle {
                key,
                latched,
                layer_key,
            } => {
                if latched {
                    let color = if layer_key {
                        lighting.layer_lock_color()
                    } else {
                        WHITE
                    };
                    lighting.start_overlay(
                        OverlayId::Key(key),
                        Span::Forever,
                        Overlay::Solid { key, color },
                        now_ms,
                    );
                } else {
                    lighting.stop_overlay(OverlayId::Key(key));
                }
            }
            Notice::ProfileBlink(key) => lighting.start_overlay(
                OverlayId::ProfileSwitch,
                Span::Milliseconds(PROFILE_BLINK_MS),
                Overlay::Blink {
                    key,
                    first: WHITE,
                    second: BLACK,
                    period_ms: PROFILE_BLINK_PERIOD_MS,
                },
                now_ms,
            ),
            Notice::StopProfileBlink => lighting.stop_overlay(OverlayId::ProfileSwitch),
            Notice::IndicatorKey { indicator, key } => {
                lighting.set_indicator_key(indicator, key, now_ms);
            }
        }
    }
}

impl Default for KeyNotices {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for KeyNotices {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KeyNotices")
            .field("queued", &self.channel.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_brightness_key_holds_until_its_own_release() {
        let up = BrightnessKeys::default().with_key(true, true);
        assert_eq!(
            up,
            BrightnessKeys {
                held: Some(true),
                pressed: Some(true),
            }
        );
        let down = up.with_key(false, true);
        assert_eq!(down.held, Some(false));
        assert_eq!(down.with_key(true, false).held, Some(false));
        assert_eq!(down.with_key(false, false).held, None);
        assert_eq!(down.with_key(false, false).pressed, Some(false));
    }

    #[test]
    fn a_brightness_key_never_fills_the_notice_queue() {
        let notices = KeyNotices::new();
        for _ in 0..KEY_NOTICE_CAPACITY {
            assert!(notices.send(Notice::ShowLayer(1)));
        }
        assert!(!notices.send(Notice::ShowLayer(2)));
        assert!(notices.send(Notice::Brightness {
            increase: false,
            held: true,
        }));
        assert!(notices.send(Notice::Brightness {
            increase: false,
            held: false,
        }));
        assert_eq!(
            notices.brightness.lock(Cell::get),
            BrightnessKeys {
                held: None,
                pressed: Some(false),
            }
        );
    }
}
