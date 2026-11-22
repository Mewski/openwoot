use openwoot::{
    hid::NkroReport,
    keys::{Functions, Key, Keyboard, Keys, Output, RapidTrigger, Switch, Tuning},
    layout::Standard,
};
use openwoot_protocol::keys::{Action, Layer};

pub(crate) const ROWS: usize = 6;
pub(crate) const COLUMNS: usize = 17;
pub(crate) const THRESHOLD: u16 = 1000;
pub(crate) const SWITCH: Switch = Switch {
    minimum: 112,
    travel_limit: 4000,
};
pub(crate) const TUNING: Tuning = Tuning {
    actuation_point: THRESHOLD,
    rapid_trigger_enabled: false,
    rapid_trigger: RapidTrigger {
        press: 200,
        release: 200,
        continuous: false,
    },
    tap_ms: 20,
    keyboard_in_gamepad_mode: false,
};
pub(crate) const KEY_A: Key = Key::new(3, 1);
pub(crate) const KEY_S: Key = Key::new(3, 2);
pub(crate) const KEY_D: Key = Key::new(3, 3);
pub(crate) const KEY_F: Key = Key::new(3, 4);

pub(crate) type Engine<F = Standard> = Keys<F, ROWS, COLUMNS>;

pub(crate) fn engine_with<F: Functions>(functions: F, keys: &[(Key, Action)]) -> Engine<F> {
    let mut engine = Engine::new(functions);
    engine.set_tuning(TUNING);
    for &(key, action) in keys {
        engine.set_switch(key, SWITCH);
        engine.set_action(key, Layer::Base, action);
    }
    engine
}

pub(crate) fn engine(keys: &[(Key, Action)]) -> Engine {
    engine_with(Standard, keys)
}

pub(crate) fn usages(output: &Output) -> Vec<u8> {
    match output.keyboard() {
        Keyboard::NKeyRollover(report) => nkro_usages(&report),
        Keyboard::Boot(report) => report
            .keys
            .iter()
            .copied()
            .filter(|usage| *usage != 0)
            .chain(
                (0..8)
                    .filter(|bit| report.modifiers.bits() & 1 << bit != 0)
                    .map(|bit| 0xE0 + bit),
            )
            .collect(),
    }
}

pub(crate) fn nkro_usages(report: &NkroReport) -> Vec<u8> {
    (0..=0xFF)
        .filter(|&usage| report.is_pressed(usage))
        .collect()
}

pub(crate) fn step<F: Functions>(
    engine: &mut Engine<F>,
    events: &[(Key, u16)],
    elapsed_ms: u16,
) -> Vec<u8> {
    let output = engine.pass(events.iter().copied(), elapsed_ms, |key| {
        events
            .iter()
            .find(|(event, _)| *event == key)
            .map_or(0, |(_, travel)| *travel)
    });
    usages(output)
}
