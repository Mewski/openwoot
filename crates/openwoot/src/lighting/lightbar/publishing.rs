use openwoot_protocol::lightbar::{Builtin, HostSignal, HostSignalIds, Signal};

use super::Bar;
use crate::{
    keys::Key,
    lighting::{
        hooks::Signals,
        indicators::LockState,
        input::{KeyEvent, TRAVEL_FULL},
        signals::ProfileState,
    },
};

impl Signals for Bar {
    fn brightness_changed(&mut self, percent: f32) {
        self.publish(Signal::Builtin(Builtin::Brightness), percent);
    }

    fn profile_changed(&mut self, profile: ProfileState) {
        self.publish(
            Signal::Builtin(Builtin::ProfileCount),
            profile.count_value(),
        );
        self.publish(
            Signal::Builtin(Builtin::ActiveProfile),
            profile.active_value(),
        );
        self.publish(
            Signal::Builtin(Builtin::ProfileColor),
            profile.color_value(),
        );
    }

    fn locks_changed(&mut self, locks: LockState) {
        self.publish(
            Signal::Builtin(Builtin::ScrollLock),
            flag(locks.scroll_lock),
        );
        self.publish(Signal::Builtin(Builtin::CapsLock), flag(locks.caps_lock));
        self.publish(Signal::Builtin(Builtin::NumLock), flag(locks.num_lock));
    }

    fn layer_changed(&mut self, layer: u8) {
        self.publish(Signal::Builtin(Builtin::ActiveLayer), f32::from(layer));
    }

    fn key_event(&mut self, key: Key, event: KeyEvent) {
        match event {
            KeyEvent::Pressed => {
                self.key_rate.note_press();
                self.held.set(key, true);
            }
            KeyEvent::Released => self.held.set(key, false),
            KeyEvent::Actuated | KeyEvent::Reset => return,
        }
        let pressed = flag(self.held.any());
        self.publish(Signal::Builtin(Builtin::KeyPressed), pressed);
    }

    fn travel_changed(&mut self, deepest: u16) {
        let level = f32::from(deepest) / f32::from(TRAVEL_FULL) * 255.0;
        self.publish(Signal::Builtin(Builtin::AnalogLevel), level);
    }

    fn tick(&mut self, now_ms: u32) {
        if let Some(rate) = self.key_rate.step(now_ms) {
            self.publish(Signal::Builtin(Builtin::KeyRate), rate);
        }
    }

    fn activate_host_signals(&mut self) {
        self.signals.activate_host();
    }

    fn set_host_signals(&mut self, values: &[HostSignal]) {
        self.signals.activate_host();
        for value in values {
            self.publish(Signal::Host(value.id), value.value);
        }
    }

    fn host_signal_ids(&self) -> HostSignalIds {
        self.signals.host_ids()
    }
}

const fn flag(set: bool) -> f32 {
    if set { 1.0 } else { 0.0 }
}
