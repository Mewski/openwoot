use heapless::Vec;
use openwoot_protocol::lightbar::{
    Builtin, HostSignal, HostSignalIds, HostSignals, MAX_ENTRIES, MAX_HOST_SIGNALS, Signal,
};

const SUBSCRIBERS: usize = 2 * MAX_ENTRIES;
const BUILTINS: usize = 11;
const FAST_SCAN_SIGNALS: u16 = 0x05F9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Subscriber {
    pub(crate) global: bool,
    pub(crate) entry: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Channel {
    value: f32,
    subscribers: Vec<Subscriber, SUBSCRIBERS>,
}

#[derive(Clone, Debug, PartialEq)]
struct HostChannel {
    id: u32,
    known: bool,
    channel: Channel,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Table {
    builtin: [Channel; BUILTINS],
    host: Vec<HostChannel, MAX_HOST_SIGNALS>,
    remembered: HostSignals,
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
        if !self.subscribers.contains(&subscriber) {
            let _ = self.subscribers.push(subscriber);
        }
    }
}

impl Table {
    pub(crate) const fn new() -> Self {
        Self {
            builtin: [const { Channel::new() }; BUILTINS],
            host: Vec::new(),
            remembered: HostSignals::new(),
            host_active: false,
        }
    }

    pub(crate) fn subscribe(&mut self, signal: Signal, subscriber: Subscriber) -> Option<f32> {
        match signal {
            Signal::Builtin(builtin) => {
                let channel = self.builtin.get_mut(index(builtin))?;
                channel.add(subscriber);
                Some(channel.value)
            }
            Signal::Host(id) => {
                if let Some(host) = self.host.iter_mut().find(|host| host.id == id) {
                    host.channel.add(subscriber);
                    return host.known.then_some(host.channel.value);
                }
                if self.host.is_full() {
                    return None;
                }
                let remembered = take_remembered(&mut self.remembered, id);
                let mut channel = Channel::new();
                channel.value = remembered.unwrap_or(0.0);
                channel.add(subscriber);
                let _ = self.host.push(HostChannel {
                    id,
                    known: remembered.is_some(),
                    channel,
                });
                remembered
            }
        }
    }

    pub(crate) fn publish_if_changed(&mut self, builtin: Builtin, value: f32) -> &[Subscriber] {
        match self.builtin.get_mut(index(builtin)) {
            Some(channel) if differs(channel.value, value) => {
                channel.value = value;
                &channel.subscribers
            }
            _ => &[],
        }
    }

    pub(crate) fn set_host(&mut self, id: u32, value: f32) -> &[Subscriber] {
        if !self.host.iter().any(|host| host.id == id) {
            remember(&mut self.remembered, HostSignal { id, value });
            return &[];
        }
        match self.host.iter_mut().find(|host| host.id == id) {
            Some(host) if !host.known || differs(host.channel.value, value) => {
                host.known = true;
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
        for host in self.host.iter().filter(|host| host.known) {
            remember(
                &mut self.remembered,
                HostSignal {
                    id: host.id,
                    value: host.channel.value,
                },
            );
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

#[expect(
    clippy::float_cmp,
    reason = "a signal changes when its value differs in any way"
)]
fn differs(stored: f32, value: f32) -> bool {
    stored != value && stored.to_bits() != value.to_bits()
}

fn remember(remembered: &mut HostSignals, value: HostSignal) {
    if let Some(kept) = remembered.iter_mut().find(|kept| kept.id == value.id) {
        kept.value = value.value;
    } else if let Err(value) = remembered.push(value) {
        remembered.rotate_left(1);
        if let Some(slot) = remembered.last_mut() {
            *slot = value;
        }
    }
}

fn take_remembered(remembered: &mut HostSignals, id: u32) -> Option<f32> {
    let mut value = None;
    remembered.retain(|kept| {
        if kept.id == id {
            value = Some(kept.value);
        }
        kept.id != id
    });
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn entry_of(entry: u8) -> Subscriber {
        Subscriber {
            global: false,
            entry,
        }
    }

    #[test]
    fn a_subscription_returns_the_current_value_and_hears_changes_only() {
        let mut table = Table::new();
        assert_eq!(table.publish_if_changed(Builtin::CapsLock, 1.0), []);
        assert_eq!(
            table.subscribe(Signal::Builtin(Builtin::CapsLock), entry_of(0)),
            Some(1.0)
        );
        assert_eq!(table.publish_if_changed(Builtin::CapsLock, 1.0), []);
        assert_eq!(
            table.publish_if_changed(Builtin::CapsLock, 0.0),
            &[entry_of(0)]
        );
    }

    #[test]
    fn a_signal_takes_every_entry_and_tells_each_of_them_once() {
        let mut table = Table::new();
        for global in [true, false] {
            for entry in (0..).take(MAX_ENTRIES) {
                let subscriber = Subscriber { global, entry };
                table.subscribe(Signal::Builtin(Builtin::KeyRate), subscriber);
                table.subscribe(Signal::Builtin(Builtin::KeyRate), subscriber);
            }
        }
        let heard = table.publish_if_changed(Builtin::KeyRate, 3.0);
        assert_eq!(heard.len(), 2 * MAX_ENTRIES);
        assert_eq!(
            heard.first(),
            Some(&Subscriber {
                global: true,
                entry: 0
            })
        );
        assert_eq!(heard.last(), Some(&entry_of(3)));
    }

    #[test]
    fn a_value_that_is_not_a_number_changes_when_its_bits_change() {
        let mut table = Table::new();
        table.subscribe(Signal::Builtin(Builtin::ProfileColor), entry_of(0));
        let color = f32::from_bits(0xFFFF_2810);
        assert!(color.is_nan());
        assert_eq!(
            table.publish_if_changed(Builtin::ProfileColor, color),
            &[entry_of(0)]
        );
        assert_eq!(table.publish_if_changed(Builtin::ProfileColor, color), []);
        assert_eq!(
            table.publish_if_changed(Builtin::ProfileColor, f32::from_bits(0xFFFF_2811)),
            &[entry_of(0)]
        );
        assert_eq!(table.publish_if_changed(Builtin::KeyRate, -0.0), []);
    }

    #[test]
    fn host_signals_register_in_order_and_keep_their_values_when_bound_again() {
        let mut table = Table::new();
        assert_eq!(table.subscribe(Signal::Host(70), entry_of(0)), None);
        table.subscribe(Signal::Host(9), entry_of(1));
        table.subscribe(Signal::Host(70), entry_of(2));
        assert_eq!(table.host_ids().as_slice(), &[70, 9]);
        assert_eq!(table.set_host(70, 0.5), &[entry_of(0), entry_of(2)]);
        assert_eq!(table.set_host(70, 0.5), []);
        assert_eq!(table.set_host(5, 0.25), []);
        assert_eq!(table.subscribe(Signal::Host(70), entry_of(0)), Some(0.5));
        table.clear();
        assert_eq!(table.host_ids().as_slice(), &[] as &[u32]);
        assert_eq!(table.subscribe(Signal::Host(9), entry_of(0)), None);
        assert_eq!(table.subscribe(Signal::Host(70), entry_of(1)), Some(0.5));
        assert_eq!(table.subscribe(Signal::Host(5), entry_of(1)), Some(0.25));
        assert_eq!(table.host_ids().as_slice(), &[9, 70, 5]);
        assert_eq!(table.set_host(70, 0.5), []);
        table.clear();
        table.clear();
        assert_eq!(table.subscribe(Signal::Host(5), entry_of(0)), Some(0.25));
    }

    #[test]
    fn the_first_value_of_a_host_signal_is_heard_even_when_it_is_zero() {
        let mut table = Table::new();
        table.subscribe(Signal::Host(1), entry_of(0));
        assert_eq!(table.set_host(1, 0.0), &[entry_of(0)]);
        assert_eq!(table.set_host(1, 0.0), []);
        assert_eq!(table.subscribe(Signal::Host(1), entry_of(1)), Some(0.0));
    }

    #[test]
    fn host_signals_beyond_the_limit_stay_unbound_across_both_lists() {
        let mut table = Table::new();
        for id in 0..30 {
            let subscriber = Subscriber {
                global: id < 15,
                entry: 0,
            };
            assert_eq!(table.subscribe(Signal::Host(id), subscriber), None);
        }
        let ids: Vec<u32, MAX_HOST_SIGNALS> = (0..20).collect();
        assert_eq!(table.host_ids(), ids);
        assert_eq!(table.set_host(25, 1.0), []);
        table.clear();
        assert_eq!(table.subscribe(Signal::Host(25), entry_of(0)), Some(1.0));
    }

    #[test]
    fn the_newest_host_values_are_kept_while_no_entry_refers_to_them() {
        let mut table = Table::new();
        table.subscribe(Signal::Host(500), entry_of(0));
        table.set_host(500, 5.0);
        table.clear();
        for id in 0..20 {
            table.set_host(id, 1.0);
        }
        table.set_host(3, 2.0);
        assert_eq!(table.subscribe(Signal::Host(500), entry_of(0)), None);
        assert_eq!(table.subscribe(Signal::Host(0), entry_of(0)), Some(1.0));
        assert_eq!(table.subscribe(Signal::Host(3), entry_of(0)), Some(2.0));
        assert_eq!(table.subscribe(Signal::Host(19), entry_of(0)), Some(1.0));
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
