use embassy_futures::block_on;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use openwoot::{
    keys::{Configurator, Key, Keyboard, Keys, Model, Shared},
    layout::{DefaultKeymap, Standard},
    lighting::KeyEvents,
    protocol::Configuration,
    runtime::{KeyNotices, KeyReports, KeyTask, Progress},
    scan::{self, Map, Position, Scanner},
    storage::{NorLayout, Partition, Settings},
    usb::{Face, HostState, Mode},
};
use openwoot_image::Region;
use openwoot_protocol::{
    advanced::{self, Behaviour},
    config::Capabilities,
    gamepad::{self, Control},
    heapless::Vec as Bounded,
    keys::{self as config, Action, Distance, Index, Layer},
    profile::Slot,
};

use super::support::{COLUMNS, ROWS, nkro_usages};
use crate::{
    scan::{Bench, Simulated},
    storage::{Nor, SECTOR},
};

type Mutual = CriticalSectionRawMutex;
type Scan = scan::Shared<Bench, 6, 15>;

const NOTHING: Action = Action::None;
const HOME_ROW: [Action; COLUMNS] = [
    NOTHING,
    Action::Keyboard(0x04),
    Action::Keyboard(0x16),
    Action::Keyboard(0x07),
    Action::Keyboard(0x09),
    Action::Keyboard(0x0A),
    Action::Function(openwoot_protocol::keys::Function::Profile(Slot::new(1))),
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
];
const LAYERS: [[[Action; COLUMNS]; ROWS]; 1] = [[
    [NOTHING; COLUMNS],
    [NOTHING; COLUMNS],
    [NOTHING; COLUMNS],
    HOME_ROW,
    [NOTHING; COLUMNS],
    [NOTHING; COLUMNS],
]];
const KEYMAP: DefaultKeymap<ROWS, COLUMNS> = DefaultKeymap {
    layers: &LAYERS,
    variant_patches: &[],
};
const NONE: u8 = 0xFF;
const MAP: Map<6, 15> = Map::from_codes([
    [
        0x61, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE,
        NONE,
    ],
    [
        0x62, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE,
        NONE,
    ],
    [
        0x63, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE,
        NONE,
    ],
    [
        0x64, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE,
        NONE,
    ],
    [
        0x65, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE,
        NONE,
    ],
    [
        0x66, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE,
        NONE,
    ],
]);
const KEY_A: Key = Key::new(3, 1);
const KEY_S: Key = Key::new(3, 2);
const PROFILE_KEY: Key = Key::new(3, 6);
const FULL: u16 = 1400;
const THREE_QUARTERS: u16 = 900;
const SETTLING_CYCLES: u32 = 6;
const MAX_CONVERSIONS: usize = 100_000;

fn input_of(key: Key) -> u8 {
    MAP.position(key).map_or(0, Position::input)
}

fn capabilities() -> Capabilities {
    Capabilities {
        key_rows: u8::try_from(ROWS).unwrap_or(u8::MAX),
        key_columns: u8::try_from(COLUMNS).unwrap_or(u8::MAX),
        profiles: 4,
        layouts: Bounded::new(),
        lightbar_leds: 0,
        host_signals: false,
        switch_models: 49,
        edition: None,
    }
}

fn storage() -> Option<Settings<Nor>> {
    let layout = NorLayout {
        capacity: 6 * SECTOR,
        settings: Region {
            start: 0,
            end: 4 * SECTOR,
        },
        device_info: None,
    };
    let partition = Partition::new(Nor::new(6), layout.settings_region());
    block_on(Settings::open(partition, false))
        .ok()
        .map(|(settings, _)| settings)
}

struct Bench80<'a> {
    scan: Scan,
    task: KeyTask<'a, Mutual>,
    keys: Keys<Standard, ROWS, COLUMNS>,
    progress: Progress,
    now_ms: u32,
}

impl<'a> Bench80<'a> {
    fn new(task: KeyTask<'a, Mutual>) -> Self {
        let scan = Scan::new();
        assert!(scan.install(Scanner::new(Simulated::default(), MAP, [0, 1, 2, 3, 4, 5])));
        let mut keys = Keys::new(Standard);
        keys.set_mode(task.face.mode);
        let mut bench = Self {
            scan,
            task,
            keys,
            progress: Progress::default(),
            now_ms: 0,
        };
        bench.scan.with(Scanner::start_sweep);
        bench.run();
        for _ in 0..SETTLING_CYCLES {
            bench.cycle();
        }
        bench
    }

    fn run(&self) {
        assert!(self.scan.start());
        let finished = (0..MAX_CONVERSIONS).find(|_| {
            self.scan.on_conversion();
            !self.scan.is_running()
        });
        assert!(finished.is_some(), "the scan never finished");
    }

    fn cycle(&mut self) {
        self.now_ms += 1;
        let now_ms = self.now_ms;
        self.scan.with(|scanner| scanner.prepare(now_ms));
        self.run();
        self.task
            .step(&self.scan, &mut self.keys, &mut self.progress, now_ms);
    }

    fn press(&self, key: Key, magnitude: u16) {
        self.scan
            .with(|scanner| scanner.converter_mut().press(input_of(key), 0, magnitude));
    }

    fn travel(&self, key: Key) -> u16 {
        MAP.position(key)
            .map_or(0, |position| self.scan.travel().get(position))
    }
}

struct World {
    shared: Shared<Mutual>,
    reports: KeyReports<Mutual>,
    events: KeyEvents,
    notices: KeyNotices,
    host: HostState,
}

impl World {
    fn new() -> Self {
        Self {
            shared: Shared::new(),
            reports: KeyReports::new(),
            events: KeyEvents::new(),
            notices: KeyNotices::new(),
            host: HostState::new(),
        }
    }

    fn task(&self, mode: Mode) -> KeyTask<'_, Mutual> {
        KeyTask {
            shared: &self.shared,
            reports: &self.reports,
            events: &self.events,
            notices: &self.notices,
            host: &self.host,
            face: Face {
                mode,
                ..Face::default()
            },
        }
    }

    fn model(&self) -> Mutex<Mutual, Model<'_, Mutual, Nor, ROWS, COLUMNS>> {
        Mutex::new(block_on(Model::open(&self.shared, storage(), KEYMAP, 0, 4)))
    }

    fn keyboard(&self) -> Option<Vec<u8>> {
        match self.reports.take().keyboard? {
            Keyboard::NKeyRollover(report) => Some(nkro_usages(&report)),
            Keyboard::Boot(_) => None,
        }
    }
}

#[test]
fn a_key_press_reaches_the_keyboard_report_at_the_end_of_the_cycle_that_sampled_it() {
    let world = World::new();
    let model = world.model();
    let mut bench = Bench80::new(world.task(Mode::NoGamepad));
    let _ = world.reports.take();
    let _ = model;
    bench.press(KEY_A, FULL);
    bench.cycle();
    assert!(bench.travel(KEY_A) > 3000, "travel {}", bench.travel(KEY_A));
    assert_eq!(world.keyboard(), Some(vec![0x04]));
    bench.cycle();
    assert_eq!(world.keyboard(), None);
    bench.press(KEY_S, FULL);
    bench.press(KEY_A, 0);
    bench.cycle();
    assert_eq!(world.keyboard(), Some(vec![0x16]));
    bench.press(KEY_S, 0);
    bench.cycle();
    assert_eq!(world.keyboard(), Some(vec![]));
}

#[test]
fn rapid_trigger_from_the_configuration_channel_applies_between_cycles() {
    let world = World::new();
    let model = world.model();
    let mut configurator = Configurator::new(&model, capabilities());
    let mut bench = Bench80::new(world.task(Mode::NoGamepad));
    let fixed: Vec<bool> = [FULL, THREE_QUARTERS, FULL]
        .into_iter()
        .map(|magnitude| {
            bench.press(KEY_A, magnitude);
            bench.cycle();
            bench
                .keys
                .actuation(KEY_A)
                .is_some_and(|actuation| actuation.is_pressed())
        })
        .collect();
    assert_eq!(fixed, [true, true, true]);
    assert!(bench.travel(KEY_A) - 400 > 1500);
    let settings = config::Settings {
        rapid_trigger_enabled: true,
        rapid_trigger: config::RapidTrigger {
            press: Distance::from_fraction(0x0800),
            release: None,
            continuous: false,
        },
        ..openwoot::keys::default_settings()
    };
    block_on(configurator.set_key_settings(Slot::new(0), settings)).unwrap();
    let rapid: Vec<bool> = [0, FULL, THREE_QUARTERS, FULL]
        .into_iter()
        .map(|magnitude| {
            bench.press(KEY_A, magnitude);
            bench.cycle();
            bench
                .keys
                .actuation(KEY_A)
                .is_some_and(|actuation| actuation.is_pressed())
        })
        .collect();
    assert_eq!(rapid, [false, true, false, true]);
    assert_eq!(
        block_on(configurator.key_settings(Slot::new(0))),
        Ok(settings)
    );
}

#[test]
fn an_advanced_key_from_the_configuration_channel_runs_in_the_key_pass() {
    let world = World::new();
    let model = world.model();
    let mut configurator = Configurator::new(&model, capabilities());
    let mut bench = Bench80::new(world.task(Mode::NoGamepad));
    let toggle = advanced::Key {
        key: Index::new(u8::try_from(KEY_A.index(COLUMNS)).unwrap()),
        layer: Layer::Base,
        behaviour: Behaviour::Toggle(Action::Keyboard(0x2C)),
    };
    block_on(configurator.set_advanced_keys(Slot::new(0), [toggle].into_iter().collect())).unwrap();
    bench.press(KEY_A, FULL);
    bench.cycle();
    bench.press(KEY_A, 0);
    bench.cycle();
    for _ in 0..10 {
        bench.cycle();
    }
    assert_eq!(nkro_of(&bench), vec![0x2C]);
    assert!(bench.keys.holds_events(KEY_A));
}

fn nkro_of(bench: &Bench80<'_>) -> Vec<u8> {
    match bench.keys.output().keyboard() {
        Keyboard::NKeyRollover(report) => nkro_usages(&report),
        Keyboard::Boot(_) => Vec::new(),
    }
}

#[test]
fn a_key_bound_to_the_gamepad_moves_the_stick_of_the_hid_gamepad_report() {
    let world = World::new();
    let model = world.model();
    let mut configurator = Configurator::new(&model, capabilities());
    let mut bench = Bench80::new(world.task(Mode::Gamepad));
    let mut bindings: Bounded<Option<Control>, 128> =
        core::iter::repeat_n(None, ROWS * COLUMNS).collect();
    if let Some(binding) = bindings.get_mut(KEY_A.index(COLUMNS)) {
        *binding = Some(Control::LeftStickRight);
    }
    block_on(configurator.set_gamepad_mapping(
        Slot::new(0),
        gamepad::Mapping {
            bindings,
            exclusive: true,
        },
    ))
    .unwrap();
    let _ = world.reports.take();
    bench.press(KEY_A, FULL);
    bench.cycle();
    let Some(openwoot::keys::GamepadReport::Hid(report)) = world.reports.take().gamepad else {
        panic!("no gamepad report");
    };
    assert!(
        report.x > openwoot::hid::GAMEPAD_STICK_CENTER + 8000,
        "x {}",
        report.x
    );
    assert_eq!(nkro_of(&bench), Vec::<u8>::new());
}

#[test]
fn a_profile_key_switches_the_profile_through_the_configuration_model() {
    let world = World::new();
    let model = world.model();
    let mut configurator = Configurator::new(&model, capabilities());
    let mut bench = Bench80::new(world.task(Mode::NoGamepad));
    bench.press(PROFILE_KEY, FULL);
    bench.cycle();
    let requested = block_on(world.shared.profile_request());
    assert_eq!(requested, 1);
    block_on(configurator.activate_profile(Slot::new(requested))).unwrap();
    bench.cycle();
    assert_eq!(bench.keys.profiles().active(), 1);
    assert_eq!(
        block_on(configurator.profiles()).map(|summary| summary.active),
        Ok(Slot::new(1))
    );
}

#[test]
fn unsaved_changes_are_dropped_by_revert_and_kept_by_save() {
    let world = World::new();
    let model = world.model();
    let mut configurator = Configurator::new(&model, capabilities());
    let changed = config::Settings {
        rapid_trigger_enabled: true,
        ..openwoot::keys::default_settings()
    };
    block_on(configurator.set_key_settings(Slot::new(0), changed)).unwrap();
    block_on(configurator.revert()).unwrap();
    assert_eq!(
        block_on(configurator.key_settings(Slot::new(0))),
        Ok(openwoot::keys::default_settings())
    );
    block_on(configurator.set_key_settings(Slot::new(0), changed)).unwrap();
    block_on(configurator.save()).unwrap();
    block_on(configurator.revert()).unwrap();
    assert_eq!(
        block_on(configurator.key_settings(Slot::new(0))),
        Ok(changed)
    );
    block_on(configurator.set_key_settings(Slot::new(2), changed)).unwrap();
    assert_eq!(
        block_on(configurator.key_settings(Slot::new(2))),
        Ok(changed)
    );
}
