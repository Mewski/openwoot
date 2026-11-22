use heapless::Vec;
use openwoot_protocol::lightbar::{Builtin, HostSignalIds, MAX_HOST_SIGNALS, Signal};

const SUBSCRIBERS: usize = 8;
const BUILTINS: usize = 11;
const FAST_SCAN_SIGNALS: u16 = 0x05F9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Input {
    Effect,
    Condition { group: u8, term: u8, right: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Subscriber {
    pub(crate) global: bool,
    pub(crate) entry: u8,
    pub(crate) input: Input,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Channel {
    value: f32,
    subscribers: Vec<Subscriber, SUBSCRIBERS>,
}

#[derive(Clone, Debug, PartialEq)]
struct HostChannel {
    id: u32,
    channel: Channel,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Table {
    builtin: [Channel; BUILTINS],
    host: Vec<HostChannel, MAX_HOST_SIGNALS>,
    host_active: bool,
}

impl Channel {
    const fn new() -> Self {
        Self {
            value: 0.0,
            subscribers: Vec::new(),
        }
    }

    fn add(&mut self, subscriber: Subscriber) {
        let _ = self.subscribers.push(subscriber);
    }
}

impl Table {
    pub(crate) const fn new() -> Self {
        Self {
            builtin: [const { Channel::new() }; BUILTINS],
            host: Vec::new(),
            host_active: false,
        }
    }

    pub(crate) fn subscribe(&mut self, signal: Signal, subscriber: Subscriber) -> f32 {
        match signal {
            Signal::Builtin(builtin) => {
                let Some(channel) = self.builtin.get_mut(index(builtin)) else {
                    return 0.0;
                };
                channel.add(subscriber);
                channel.value
            }
            Signal::Host(id) => {
                if let Some(host) = self.host.iter_mut().find(|host| host.id == id) {
                    host.channel.add(subscriber);
                } else {
                    let mut channel = Channel::new();
                    channel.add(subscriber);
                    let _ = self.host.push(HostChannel { id, channel });
                }
                0.0
            }
        }
    }

    #[expect(
        clippy::float_cmp,
        reason = "a signal changes when its value differs in any way, as in the reference firmware"
    )]
    pub(crate) fn publish_if_changed(&mut self, builtin: Builtin, value: f32) -> &[Subscriber] {
        match self.builtin.get_mut(index(builtin)) {
            Some(channel) if channel.value != value => {
                channel.value = value;
                &channel.subscribers
            }
            _ => &[],
        }
    }

    #[expect(
        clippy::float_cmp,
        reason = "a signal changes when its value differs in any way, as in the reference firmware"
    )]
    pub(crate) fn set_host(&mut self, id: u32, value: f32) -> &[Subscriber] {
        match self.host.iter_mut().find(|host| host.id == id) {
            Some(host) if host.channel.value != value => {
                host.channel.value = value;
                &host.channel.subscribers
            }
            _ => &[],
        }
    }

    pub(crate) fn clear(&mut self) {
        for channel in &mut self.builtin {
            channel.subscribers.clear();
        }
        self.host.clear();
    }

    #[must_use]
    pub(crate) fn host_ids(&self) -> HostSignalIds {
        self.host.iter().map(|host| host.id).collect()
    }

    pub(crate) const fn activate_host(&mut self) {
        self.host_active = true;
    }

    #[must_use]
    pub(crate) const fn host_active(&self) -> bool {
        self.host_active
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub(crate) const fn allowed_in_fast_scan(signal: Signal) -> bool {
    match signal {
        Signal::Builtin(builtin) => FAST_SCAN_SIGNALS & (1 << index(builtin)) != 0,
        Signal::Host(_) => false,
    }
}

const fn index(builtin: Builtin) -> usize {
    match builtin {
        Builtin::ActiveProfile => 0,
        Builtin::AnalogLevel => 1,
        Builtin::KeyPressed => 2,
        Builtin::CapsLock => 3,
        Builtin::ScrollLock => 4,
        Builtin::NumLock => 5,
        Builtin::ProfileCount => 6,
        Builtin::ActiveLayer => 7,
        Builtin::Brightness => 8,
        Builtin::KeyRate => 9,
        Builtin::ProfileColor => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn effect_of(entry: u8) -> Subscriber {
        Subscriber {
            global: false,
            entry,
            input: Input::Effect,
        }
    }

    #[test]
    fn a_subscription_returns_the_current_value_and_hears_changes_only() {
        let mut table = Table::new();
        assert_eq!(table.publish_if_changed(Builtin::CapsLock, 1.0), []);
        assert_eq!(
            table.subscribe(Signal::Builtin(Builtin::CapsLock), effect_of(0)),
            1.0
        );
        assert_eq!(table.publish_if_changed(Builtin::CapsLock, 1.0), []);
        assert_eq!(
            table.publish_if_changed(Builtin::CapsLock, 0.0),
            &[effect_of(0)]
        );
    }

    #[test]
    fn a_signal_keeps_at_most_eight_subscribers() {
        let mut table = Table::new();
        for entry in 0..10 {
            table.subscribe(Signal::Builtin(Builtin::KeyRate), effect_of(entry));
        }
        assert_eq!(
            table.publish_if_changed(Builtin::KeyRate, 3.0).len(),
            SUBSCRIBERS
        );
    }

    #[test]
    fn host_signals_register_in_order_and_are_dropped_with_the_subscribers() {
        let mut table = Table::new();
        assert_eq!(table.subscribe(Signal::Host(70), effect_of(0)), 0.0);
        table.subscribe(Signal::Host(9), effect_of(1));
        table.subscribe(Signal::Host(70), effect_of(2));
        assert_eq!(table.host_ids().as_slice(), &[70, 9]);
        assert_eq!(table.set_host(70, 0.5), &[effect_of(0), effect_of(2)]);
        assert_eq!(table.set_host(70, 0.5), []);
        assert_eq!(table.set_host(5, 0.5), []);
        assert_eq!(table.subscribe(Signal::Host(70), effect_of(0)), 0.0);
        table.clear();
        assert_eq!(table.host_ids().as_slice(), &[] as &[u32]);
    }

    #[test]
    fn fast_scan_keeps_the_signals_that_do_not_follow_the_keys() {
        assert!(allowed_in_fast_scan(Signal::Builtin(
            Builtin::ActiveProfile
        )));
        assert!(allowed_in_fast_scan(Signal::Builtin(Builtin::ProfileColor)));
        assert!(!allowed_in_fast_scan(Signal::Builtin(Builtin::AnalogLevel)));
        assert!(!allowed_in_fast_scan(Signal::Builtin(Builtin::KeyPressed)));
        assert!(!allowed_in_fast_scan(Signal::Builtin(Builtin::KeyRate)));
        assert!(!allowed_in_fast_scan(Signal::Host(0)));
    }
}
