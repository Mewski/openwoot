use core::cell::Cell;

use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
};

use crate::{
    keys::Notice,
    lighting::{BLACK, Lighting, Overlay, OverlayId, Span},
};

const KEY_NOTICE_CAPACITY: usize = 16;

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

pub struct KeyNotices {
    channel: Channel<CriticalSectionRawMutex, Notice, KEY_NOTICE_CAPACITY>,
    repeat: Mutex<CriticalSectionRawMutex, Cell<Option<Repeat>>>,
}

impl KeyNotices {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            channel: Channel::new(),
            repeat: Mutex::new(Cell::new(None)),
        }
    }

    pub fn send(&self, notice: Notice) -> bool {
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
        let repeat = self.repeat.lock(Cell::get);
        if let Some(repeat) = repeat
            && now_ms.wrapping_sub(repeat.due_ms).cast_signed() >= 0
        {
            lighting.step_brightness(repeat.increase, now_ms);
            self.repeat.lock(|cell| {
                cell.set(Some(Repeat {
                    due_ms: repeat.due_ms.wrapping_add(BRIGHTNESS_REPEAT_MS),
                    ..repeat
                }));
            });
        }
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
            Notice::Brightness { increase, held } => {
                let repeat = held.then_some(Repeat {
                    increase,
                    due_ms: now_ms.wrapping_add(BRIGHTNESS_REPEAT_MS),
                });
                self.repeat.lock(|cell| cell.set(repeat));
            }
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
