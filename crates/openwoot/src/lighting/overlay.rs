use heapless::Vec;

use super::{canvas::Canvas, config::Rgb, hooks::Lightbar, truncate::low_16};
use crate::keys::Key;

pub const FADE_FULL: u8 = u8::MAX;
pub(super) const MAX_LIGHTBAR_LEDS: usize = 16;
pub(super) const CLASS_KEYS: u8 = 0x01;
pub(super) const CLASS_GLOBAL: u8 = 0x02;
pub(super) const CLASS_ALL: u8 = CLASS_KEYS | CLASS_GLOBAL;

const SLOTS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayId {
    ProfileSwitch,
    CapsLock,
    LayerLock,
    ScrollLock,
    NumLock,
    HostLightbar,
    IdleFade,
    WakeFade,
    Key(Key),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Solid {
        key: Key,
        color: Rgb,
    },
    Blink {
        key: Key,
        first: Rgb,
        second: Rgb,
        period_ms: u16,
    },
    Fade {
        from: u8,
        to: u8,
        duration_ms: u16,
    },
    Lightbar([Rgb; MAX_LIGHTBAR_LEDS]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    Forever,
    Milliseconds(u16),
}

impl Overlay {
    pub(super) const fn fade(from: u8, to: u8, duration_ms: u16) -> Self {
        Self::Fade {
            from,
            to,
            duration_ms,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Slot {
    id: OverlayId,
    class: u8,
    restart: bool,
    start_ms: u32,
    span: Span,
    overlay: Overlay,
    progress_ms: u16,
}

impl Slot {
    const fn finished(&self) -> bool {
        matches!(self.span, Span::Milliseconds(0))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Overlays {
    slots: Vec<Slot, SLOTS>,
    dirty_classes: u8,
}

pub(super) struct Pass<'a, const ROWS: usize, const COLUMNS: usize> {
    pub(super) restore: Option<&'a [[Rgb; COLUMNS]; ROWS]>,
    pub(super) classes: u8,
    pub(super) now_ms: u32,
    pub(super) elapsed_ms: u16,
}

impl Overlays {
    pub(super) const fn new() -> Self {
        Self {
            slots: Vec::new(),
            dirty_classes: 0,
        }
    }

    pub(super) fn start(
        &mut self,
        id: OverlayId,
        span: Span,
        overlay: Overlay,
        now_ms: u32,
        lightbar_class: u8,
    ) {
        let class = match overlay {
            Overlay::Solid { .. } | Overlay::Blink { .. } => CLASS_KEYS,
            Overlay::Fade { .. } => CLASS_GLOBAL,
            Overlay::Lightbar(_) => lightbar_class,
        };
        let slot = Slot {
            id,
            class,
            restart: true,
            start_ms: now_ms,
            span,
            overlay,
            progress_ms: 0,
        };
        if let Some(running) = self.slots.iter_mut().find(|slot| slot.id == id) {
            self.dirty_classes |= running.class;
            *running = slot;
        } else if self.slots.push(slot).is_err() {
            return;
        }
        self.dirty_classes |= class;
    }

    pub(super) fn stop(&mut self, id: OverlayId) {
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.id == id) {
            slot.span = Span::Milliseconds(0);
        }
    }

    pub(super) fn is_running(&self, id: OverlayId) -> bool {
        self.slots
            .iter()
            .any(|slot| slot.id == id && !slot.finished())
    }

    pub(super) fn needs_render(&self, since_ms: u16) -> bool {
        if self.dirty_classes != 0 {
            return true;
        }
        self.slots
            .iter()
            .filter(|slot| slot.class != 0)
            .any(|slot| {
                if slot.finished() {
                    return true;
                }
                match slot.overlay {
                    Overlay::Blink { period_ms, .. } => {
                        let phase = u32::from(slot.progress_ms);
                        let since = u32::from(since_ms);
                        let period = u32::from(period_ms);
                        let half = period >> 1;
                        since + phase > period || (phase <= half && since + phase > half)
                    }
                    Overlay::Fade { duration_ms, .. } => duration_ms > slot.progress_ms,
                    Overlay::Solid { .. } | Overlay::Lightbar(_) => false,
                }
            })
    }

    pub(super) fn idle_fade_done(&self) -> bool {
        self.slots
            .iter()
            .find(|slot| slot.id == OverlayId::IdleFade)
            .is_none_or(|slot| match slot.overlay {
                Overlay::Fade { duration_ms, .. } => duration_ms <= slot.progress_ms,
                Overlay::Solid { .. } | Overlay::Lightbar(_) => true,
                Overlay::Blink { .. } => false,
            })
    }

    pub(super) fn render<const ROWS: usize, const COLUMNS: usize>(
        &mut self,
        canvas: &mut impl Canvas,
        lightbar: &mut impl Lightbar,
        pass: &Pass<'_, ROWS, COLUMNS>,
        fade_level: &mut u8,
    ) {
        let mut index = 0;
        while let Some(slot) = self.slots.get_mut(index) {
            if pass.classes & slot.class == 0 {
                index += 1;
                continue;
            }
            if slot.finished() {
                finish(canvas, lightbar, pass.restore, slot, fade_level);
                self.slots.swap_remove(index);
                continue;
            }
            let elapsed = if slot.restart {
                slot.restart = false;
                low_16(pass.now_ms.wrapping_sub(slot.start_ms))
            } else {
                pass.elapsed_ms
            };
            draw(canvas, lightbar, slot, elapsed, fade_level);
            if let Span::Milliseconds(remaining) = slot.span {
                slot.span = Span::Milliseconds(remaining.saturating_sub(elapsed));
            }
            index += 1;
        }
        self.dirty_classes &= !pass.classes;
    }
}

fn finish<const ROWS: usize, const COLUMNS: usize>(
    canvas: &mut impl Canvas,
    lightbar: &mut impl Lightbar,
    restore: Option<&[[Rgb; COLUMNS]; ROWS]>,
    slot: &Slot,
    fade_level: &mut u8,
) {
    match slot.overlay {
        Overlay::Solid { key, .. } | Overlay::Blink { key, .. } => {
            if let Some(color) = restore
                .and_then(|colors| colors.get(key.row()))
                .and_then(|row| row.get(key.column()))
            {
                canvas.set_key(key.row(), key.column(), *color);
            }
        }
        Overlay::Fade { .. } => *fade_level = FADE_FULL,
        Overlay::Lightbar(_) => lightbar.clear_overlay(canvas),
    }
}

fn draw(
    canvas: &mut impl Canvas,
    lightbar: &mut impl Lightbar,
    slot: &mut Slot,
    elapsed: u16,
    fade_level: &mut u8,
) {
    match slot.overlay {
        Overlay::Solid { key, color } => canvas.set_key(key.row(), key.column(), color),
        Overlay::Blink {
            key,
            first,
            second,
            period_ms,
        } => {
            let color = if period_ms == 0 {
                first
            } else {
                let mut phase = elapsed % period_ms;
                if phase == 0 && elapsed != 0 {
                    phase = period_ms;
                }
                let mut phase = u32::from(phase) + u32::from(slot.progress_ms);
                if phase >= u32::from(period_ms) {
                    phase -= u32::from(period_ms);
                }
                slot.progress_ms = low_16(phase);
                if slot.progress_ms > period_ms >> 1 {
                    second
                } else {
                    first
                }
            };
            canvas.set_key(key.row(), key.column(), color);
        }
        Overlay::Fade {
            from,
            to,
            duration_ms,
        } => {
            let total = u32::from(slot.progress_ms) + u32::from(elapsed);
            if total > u32::from(duration_ms) || duration_ms == 0 {
                *fade_level = to;
                slot.progress_ms = slot.progress_ms.max(duration_ms);
            } else {
                slot.progress_ms = low_16(total);
                let progress = (255 * total / u32::from(duration_ms)).to_le_bytes()[0];
                let level = ((i32::from(from) << 8)
                    + i32::from(progress) * (i32::from(to) - i32::from(from)))
                    >> 8;
                *fade_level = level.to_le_bytes()[0];
            }
        }
        Overlay::Lightbar(colors) => lightbar.draw_overlay(canvas, &colors),
    }
}
