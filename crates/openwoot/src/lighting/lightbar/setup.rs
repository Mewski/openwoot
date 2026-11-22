use openwoot_protocol::lightbar::{Builtin, Effect, Entries, Entry, MAX_ENTRIES, Settings, Signal};

use super::{
    background::background_of,
    binding::{Origin, bind},
    condition::{evaluate, needs_render},
    paint::Strip,
    state::{EffectState, EntryState},
};
use crate::lighting::{
    canvas::Canvas,
    color::opaque,
    signals::{Input, Subscriber, Table, color_of},
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Setup {
    pub(super) profile: Settings,
    pub(super) global: Entries,
    profile_states: [EntryState; MAX_ENTRIES],
    global_states: [EntryState; MAX_ENTRIES],
}

pub(super) struct Pass<'a> {
    pub(super) strip: Strip<'a>,
    pub(super) host_signals: bool,
}

impl Setup {
    pub(super) const fn new() -> Self {
        Self {
            profile: Settings {
                enabled: false,
                entries: Entries::new(),
                use_global: false,
                brightness: None,
            },
            global: Entries::new(),
            profile_states: [EntryState::new(); MAX_ENTRIES],
            global_states: [EntryState::new(); MAX_ENTRIES],
        }
    }

    pub(super) fn bind(&mut self, signals: &mut Table, fast_scan: bool) -> bool {
        let mut bound = false;
        if self.profile.use_global {
            for ((entry, state), number) in self.global.iter().zip(&mut self.global_states).zip(0..)
            {
                let origin = Origin {
                    global: true,
                    entry: number,
                };
                bound |= bind(signals, origin, entry, state, fast_scan);
            }
        }
        for ((entry, state), number) in self
            .profile
            .entries
            .iter()
            .zip(&mut self.profile_states)
            .zip(0..)
        {
            let origin = Origin {
                global: false,
                entry: number,
            };
            bound |= bind(signals, origin, entry, state, fast_scan);
        }
        bound
    }

    pub(super) fn needs_render(&self, since_render_ms: u16, host_control: bool) -> bool {
        let needs = |(entry, state): (&Entry, &EntryState)| {
            entry_needs_render(entry, state, since_render_ms, host_control)
        };
        (self.profile.use_global && self.global.iter().zip(&self.global_states).any(needs))
            || self
                .profile
                .entries
                .iter()
                .zip(&self.profile_states)
                .any(needs)
    }

    pub(super) fn render(&mut self, canvas: &mut impl Canvas, pass: &Pass<'_>, elapsed_ms: u16) {
        if self.profile.use_global {
            for (entry, state) in self.global.iter().zip(&mut self.global_states) {
                render_entry(canvas, pass, entry, state, elapsed_ms);
            }
        }
        for (entry, state) in self.profile.entries.iter().zip(&mut self.profile_states) {
            render_entry(canvas, pass, entry, state, elapsed_ms);
        }
    }

    #[expect(
        clippy::float_cmp,
        reason = "an input changes when its value differs in any way"
    )]
    pub(super) fn notify(&mut self, subscriber: Subscriber, signal: Signal, value: f32) {
        let (entries, states) = if subscriber.global {
            if !self.profile.use_global {
                return;
            }
            (&self.global, &mut self.global_states)
        } else {
            (&self.profile.entries, &mut self.profile_states)
        };
        let index = usize::from(subscriber.entry);
        let (Some(entry), Some(state)) = (entries.get(index), states.get_mut(index)) else {
            return;
        };
        if !state.bound {
            return;
        }
        match subscriber.input {
            Input::Effect => notify_effect(entry, &mut state.effect, signal, value),
            Input::Condition { group, term, right } => {
                if entry.condition.is_none() {
                    return;
                }
                let condition = &mut state.condition;
                let Some(term) = condition.term_mut(group, term) else {
                    return;
                };
                let input = if right {
                    &mut term.right
                } else {
                    &mut term.left
                };
                if *input != value {
                    *input = value;
                    term.changed = true;
                    condition.dirty = true;
                }
            }
        }
    }
}

#[expect(
    clippy::float_cmp,
    reason = "an input changes when its value differs in any way"
)]
fn notify_effect(entry: &Entry, effect: &mut EffectState, signal: Signal, value: f32) {
    match (&entry.effect, effect) {
        (
            Effect::Meter(_),
            EffectState::Meter {
                value: shown,
                dirty,
            },
        ) => {
            if *shown != value {
                *shown = value;
                *dirty = true;
            }
        }
        (Effect::ProfileIndicator(indicator), EffectState::Indicator(view)) => match signal {
            Signal::Builtin(Builtin::ActiveProfile) => {
                if view.profile != value {
                    view.profile = value;
                    view.dirty = true;
                }
            }
            Signal::Builtin(Builtin::ProfileCount) => {
                if view.count != value {
                    view.count = value;
                    view.dirty = true;
                }
            }
            Signal::Builtin(Builtin::ProfileColor) => {
                let color = color_of(value);
                if color != view.color {
                    view.color = color;
                    view.dirty = true;
                    view.background = background_of(indicator.background, color);
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn entry_needs_render(
    entry: &Entry,
    state: &EntryState,
    since_render_ms: u16,
    host_control: bool,
) -> bool {
    if !state.bound {
        return false;
    }
    if let Some(condition) = &entry.condition
        && needs_render(condition, &state.condition, since_render_ms)
    {
        return true;
    }
    match (&entry.effect, &state.effect) {
        (Effect::Meter(_), EffectState::Meter { dirty, .. }) => *dirty,
        (Effect::ProfileIndicator(_), EffectState::Indicator(view)) => view.dirty,
        (Effect::Mirror(_), _) => host_control,
        (Effect::Blink(_), _) => since_render_ms != 0,
        _ => false,
    }
}

fn render_entry(
    canvas: &mut impl Canvas,
    pass: &Pass<'_>,
    entry: &Entry,
    state: &mut EntryState,
    elapsed_ms: u16,
) {
    if !state.bound {
        return;
    }
    let mut restart = false;
    if let Some(condition) = &entry.condition {
        let outcome = evaluate(condition, &mut state.condition, u32::from(elapsed_ms));
        if !outcome.shown {
            return;
        }
        restart = outcome.changed;
    }
    let strip = pass.strip;
    match (&entry.effect, &mut state.effect) {
        (Effect::Meter(meter), EffectState::Meter { value, dirty }) => {
            if matches!(meter.signal, Signal::Host(_)) && !pass.host_signals {
                return;
            }
            strip.meter(canvas, meter, *value);
            *dirty = false;
        }
        (Effect::ProfileIndicator(indicator), EffectState::Indicator(view)) => {
            if strip.indicator(canvas, *indicator, view) {
                view.dirty = false;
            }
        }
        (Effect::Mirror(mirror), _) => strip.mirror(canvas, *mirror),
        (Effect::Blink(blink), EffectState::Blink { phase_ms }) => {
            strip.blink(canvas, blink, phase_ms, elapsed_ms, restart);
        }
        (Effect::Solid(solid), _) => strip.fill(canvas, solid.target, opaque(solid.color)),
        (Effect::Custom(custom), _) => strip.custom(canvas, custom),
        _ => {}
    }
}
