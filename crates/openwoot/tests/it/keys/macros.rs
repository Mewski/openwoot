use core::{pin::pin, task::Poll};
use std::collections::{HashMap, HashSet};

use embassy_futures::poll_once;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use openwoot::{
    hid::MouseReport,
    keys::{self, Key, Parts},
    macros::{self, Drained, Header},
    profile::Configurator,
    protocol::Configuration,
    usb::Mode as UsbMode,
};
use openwoot_protocol::{
    device::Layout,
    keys::{Action, Function, Keymap, Layer, MouseButton, SystemControl},
    macros::{Encoded, Macro, Mode, Name, Step, Text},
    profile::Slot,
    wire,
};

use super::{
    pipeline::{Bench80, FULL, World, capabilities, storage},
    support::{COLUMNS, Engine, KEY_A, KEY_D, KEY_S, engine, usages},
};

const PRESSED: u16 = 2000;
const TAKEN: Drained = Drained {
    keyboard: true,
    controls: true,
};
const WAITING: Drained = Drained {
    keyboard: false,
    controls: false,
};
const FN: Key = Key::new(5, 12);
const SHIFT: u8 = 0xE1;
const LETTER_A: u8 = 0x04;
const LETTER_B: u8 = 0x05;
const VOLUME_UP: u16 = 0xE9;

struct Bench {
    keys: Engine,
    playback: macros::Shared<NoopRawMutex>,
    library: HashMap<u8, Vec<u8>>,
    layout: Layout,
    bound: Vec<Key>,
    down: HashSet<Key>,
}

impl Bench {
    fn new(bindings: &[(Key, Action)]) -> Self {
        Self {
            keys: engine(bindings),
            playback: macros::Shared::new(),
            library: HashMap::new(),
            layout: Layout::Ansi,
            bound: bindings.iter().map(|(key, _)| *key).collect(),
            down: HashSet::new(),
        }
    }

    #[expect(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        reason = "every macro a test builds fits the buffer"
    )]
    fn with(mut self, index: u8, value: &Macro) -> Self {
        let mut buffer = [0; 4096];
        let len = wire::encode(value, &mut buffer).unwrap();
        self.library.insert(index, buffer[..len].to_vec());
        self
    }

    fn on(mut self, layout: Layout) -> Self {
        self.layout = layout;
        self
    }

    fn press(&mut self, key: Key) {
        self.down.insert(key);
    }

    fn release(&mut self, key: Key) {
        self.down.remove(&key);
    }

    fn pass(&mut self, drained: Drained) -> Vec<u8> {
        let events: Vec<(Key, u16)> = self
            .bound
            .iter()
            .map(|key| (*key, if self.down.contains(key) { PRESSED } else { 0 }))
            .collect();
        self.keys.pass(events.iter().copied(), 1, |key| {
            events
                .iter()
                .find(|(event, _)| *event == key)
                .map_or(0, |(_, travel)| *travel)
        });
        self.keys.play(&self.playback, drained);
        self.serve();
        usages(self.keys.output())
    }

    fn passes(&mut self, count: usize) -> Vec<Vec<u8>> {
        (0..count).map(|_| self.pass(TAKEN)).collect()
    }

    fn mouse(&self) -> MouseReport {
        *self.keys.output().mouse()
    }

    fn serve(&self) {
        let Poll::Ready(index) = poll_once(pin!(self.playback.wanted())) else {
            return;
        };
        match self
            .library
            .get(&index)
            .and_then(|bytes| Encoded::parse(bytes))
        {
            Some(value) => {
                let header = Header {
                    mode: value.mode(),
                    layout: self.layout,
                    steps: value.step_count(),
                };
                self.playback.fill(index, header, value.encoded_steps());
            }
            None => self.playback.missing(index),
        }
    }
}

fn played(mode: Mode, steps: impl IntoIterator<Item = Step>) -> Macro {
    Macro {
        name: Name::new(),
        mode,
        steps: steps.into_iter().collect(),
    }
}

fn tap(usage: u8) -> Step {
    Step::Tap(Action::Keyboard(usage))
}

fn press(usage: u8) -> Step {
    Step::Press(Action::Keyboard(usage))
}

fn release(usage: u8) -> Step {
    Step::Release(Action::Keyboard(usage))
}

#[expect(
    clippy::unwrap_used,
    reason = "every text a test types fits a text step"
)]
fn text(value: &str) -> Step {
    Step::Text(Text::try_from(value).unwrap())
}

fn motion(mouse: MouseReport) -> (u8, i8, i8, i8, i8) {
    (mouse.buttons, mouse.x, mouse.y, mouse.wheel, mouse.pan)
}

fn typed(layout: Layout, value: &str) -> Vec<Vec<u8>> {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))])
        .with(0, &played(Mode::Once, [text(value)]))
        .on(layout);
    bench.press(KEY_A);
    bench
        .passes(4 * value.len() + 4)
        .into_iter()
        .filter(|keys| keys.iter().any(|usage| *usage != SHIFT))
        .collect()
}

#[test]
fn a_once_macro_plays_one_change_per_report_and_stops_at_its_end() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))]).with(
        0,
        &played(Mode::Once, [press(SHIFT), tap(LETTER_A), release(SHIFT)]),
    );
    bench.press(KEY_A);
    assert_eq!(
        bench.passes(7),
        [
            vec![],
            vec![SHIFT],
            vec![LETTER_A, SHIFT],
            vec![SHIFT],
            vec![],
            vec![],
            vec![]
        ]
    );
    bench.release(KEY_A);
    bench.pass(TAKEN);
    bench.press(KEY_A);
    assert_eq!(
        bench.passes(3),
        [vec![], vec![SHIFT], vec![LETTER_A, SHIFT]]
    );
}

#[test]
fn a_delay_holds_the_next_step_for_its_milliseconds() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))]).with(
        0,
        &played(Mode::Once, [tap(LETTER_A), Step::Delay(5), tap(LETTER_B)]),
    );
    bench.press(KEY_A);
    assert_eq!(
        bench.passes(10),
        [
            vec![],
            vec![LETTER_A],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![LETTER_B],
            vec![],
            vec![]
        ]
    );
}

#[test]
fn a_while_held_macro_repeats_while_its_key_is_held_and_finishes_the_pass_in_progress() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))])
        .with(0, &played(Mode::WhileHeld, [tap(LETTER_A), Step::Delay(2)]));
    bench.press(KEY_A);
    assert_eq!(
        bench.passes(6),
        [
            vec![],
            vec![LETTER_A],
            vec![],
            vec![],
            vec![],
            vec![LETTER_A]
        ]
    );
    bench.release(KEY_A);
    assert_eq!(
        bench.passes(6),
        [vec![], vec![], vec![], vec![], vec![], vec![]]
    );
    bench.press(KEY_A);
    bench.pass(TAKEN);
    bench.release(KEY_A);
    assert_eq!(
        bench.passes(6),
        [vec![LETTER_A], vec![], vec![], vec![], vec![], vec![]]
    );
}

#[test]
fn a_toggle_macro_repeats_from_one_press_and_stops_at_once_on_the_next() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))]).with(
        0,
        &played(
            Mode::Toggle,
            [
                press(LETTER_A),
                Step::Delay(3),
                release(LETTER_A),
                Step::Delay(3),
            ],
        ),
    );
    bench.press(KEY_A);
    bench.pass(TAKEN);
    bench.release(KEY_A);
    assert_eq!(
        bench.passes(8),
        [
            vec![LETTER_A],
            vec![LETTER_A],
            vec![LETTER_A],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![LETTER_A]
        ]
    );
    bench.press(KEY_A);
    assert_eq!(bench.pass(TAKEN), []);
    bench.release(KEY_A);
    assert_eq!(bench.passes(8), vec![Vec::<u8>::new(); 8]);
}

#[test]
fn keys_a_pass_pressed_and_did_not_release_are_released_at_its_end() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))]).with(
        0,
        &played(
            Mode::Once,
            [press(LETTER_A), press(LETTER_B), Step::Delay(2)],
        ),
    );
    bench.press(KEY_A);
    assert_eq!(
        bench.passes(6),
        [
            vec![],
            vec![LETTER_A],
            vec![LETTER_A, LETTER_B],
            vec![LETTER_A, LETTER_B],
            vec![],
            vec![]
        ]
    );
}

#[test]
fn text_is_typed_key_by_key_with_shift_for_the_shifted_characters() {
    let mut bench =
        Bench::new(&[(KEY_A, Action::Macro(0))]).with(0, &played(Mode::Once, [text("aA!\n")]));
    bench.press(KEY_A);
    assert_eq!(
        bench.passes(12),
        [
            vec![],
            vec![LETTER_A],
            vec![],
            vec![SHIFT],
            vec![LETTER_A, SHIFT],
            vec![],
            vec![SHIFT],
            vec![0x1E, SHIFT],
            vec![],
            vec![0x28],
            vec![],
            vec![]
        ]
    );
}

#[test]
fn text_follows_the_operating_system_layout_of_the_layout_in_effect() {
    assert_eq!(
        typed(Layout::Ansi, "\"@#\\|~"),
        [
            vec![0x34, SHIFT],
            vec![0x1F, SHIFT],
            vec![0x20, SHIFT],
            vec![0x31],
            vec![0x31, SHIFT],
            vec![0x35, SHIFT]
        ]
    );
    assert_eq!(
        typed(Layout::Iso, "\"@#\\|~"),
        [
            vec![0x1F, SHIFT],
            vec![0x34, SHIFT],
            vec![0x31],
            vec![0x64],
            vec![0x64, SHIFT],
            vec![0x31, SHIFT]
        ]
    );
    assert_eq!(
        typed(Layout::Jis, "&'_\\|="),
        [
            vec![0x23, SHIFT],
            vec![0x24, SHIFT],
            vec![0x87, SHIFT],
            vec![0x87],
            vec![0x89, SHIFT],
            vec![0x2D, SHIFT]
        ]
    );
}

#[test]
fn mouse_movement_goes_out_in_steps_of_127_with_a_still_report_between_them() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))]).with(
        0,
        &played(
            Mode::Once,
            [
                Step::MouseMove { x: 300, y: -10 },
                Step::Scroll {
                    vertical: -3,
                    horizontal: 2,
                },
                Step::Tap(Action::Mouse(MouseButton::Left)),
            ],
        ),
    );
    bench.press(KEY_A);
    let reports: Vec<_> = (0..11)
        .map(|_| {
            bench.pass(TAKEN);
            motion(bench.mouse())
        })
        .collect();
    assert_eq!(
        reports,
        [
            (0, 0, 0, 0, 0),
            (0, 127, -10, 0, 0),
            (0, 0, 0, 0, 0),
            (0, 127, 0, 0, 0),
            (0, 0, 0, 0, 0),
            (0, 46, 0, 0, 0),
            (0, 0, 0, 0, 0),
            (0, 0, 0, -3, 2),
            (1, 0, 0, 0, 0),
            (0, 0, 0, 0, 0),
            (0, 0, 0, 0, 0)
        ]
    );
}

#[test]
fn the_next_change_waits_until_the_host_took_the_report_of_the_last_one() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))])
        .with(0, &played(Mode::Once, [tap(LETTER_A), tap(LETTER_B)]));
    bench.press(KEY_A);
    bench.pass(TAKEN);
    assert_eq!(bench.pass(TAKEN), [LETTER_A]);
    assert_eq!(bench.pass(WAITING), [LETTER_A]);
    assert_eq!(bench.pass(WAITING), [LETTER_A]);
    assert_eq!(bench.pass(TAKEN), Vec::<u8>::new());
    assert_eq!(bench.pass(WAITING), Vec::<u8>::new());
    assert_eq!(bench.pass(TAKEN), [LETTER_B]);
}

#[test]
fn a_host_that_reads_no_keyboard_report_holds_back_no_mouse_or_media_step() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))]).with(
        0,
        &played(
            Mode::Once,
            [
                Step::Tap(Action::Mouse(MouseButton::Right)),
                Step::Tap(Action::Consumer(VOLUME_UP)),
                Step::Tap(Action::System(SystemControl::Sleep)),
            ],
        ),
    );
    let controls_only = Drained {
        keyboard: false,
        controls: true,
    };
    bench.press(KEY_A);
    let steps: Vec<_> = (0..8)
        .map(|_| {
            bench.pass(controls_only);
            let output = bench.keys.output();
            (
                output.mouse().buttons,
                output.consumer_usages().to_vec(),
                output.system().control.is_some(),
            )
        })
        .collect();
    assert_eq!(
        steps,
        [
            (0, vec![], false),
            (2, vec![], false),
            (0, vec![], false),
            (0, vec![VOLUME_UP], false),
            (0, vec![], false),
            (0, vec![], true),
            (0, vec![], false),
            (0, vec![], false)
        ]
    );
}

#[test]
fn triggering_another_macro_releases_what_the_first_holds_and_plays_the_second() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0)), (KEY_S, Action::Macro(1))])
        .with(
            0,
            &played(
                Mode::Once,
                [press(LETTER_A), Step::Delay(100), release(LETTER_A)],
            ),
        )
        .with(1, &played(Mode::Once, [tap(LETTER_B)]));
    bench.press(KEY_A);
    assert_eq!(bench.passes(3), [vec![], vec![LETTER_A], vec![LETTER_A]]);
    bench.release(KEY_A);
    bench.press(KEY_S);
    assert_eq!(
        bench.passes(5),
        [vec![], vec![LETTER_B], vec![], vec![], vec![]]
    );
}

#[test]
fn a_macro_that_is_not_stored_plays_nothing_and_leaves_the_next_one_alone() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(5)), (KEY_S, Action::Macro(1))])
        .with(1, &played(Mode::Once, [tap(LETTER_B)]));
    bench.press(KEY_A);
    assert_eq!(bench.passes(4), vec![Vec::<u8>::new(); 4]);
    bench.release(KEY_A);
    bench.press(KEY_S);
    assert_eq!(bench.passes(3), [vec![], vec![LETTER_B], vec![]]);
}

#[test]
fn a_macro_never_plays_another_macro_a_layer_or_a_function() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))])
        .with(
            0,
            &played(
                Mode::Once,
                [
                    Step::Tap(Action::Macro(1)),
                    Step::Press(Action::Layer(Layer::One)),
                    Step::Tap(Action::Function(Function::BrightnessUp)),
                    tap(LETTER_A),
                ],
            ),
        )
        .with(1, &played(Mode::Once, [tap(LETTER_B)]));
    bench.press(KEY_A);
    assert_eq!(
        bench.passes(5),
        [vec![], vec![LETTER_A], vec![], vec![], vec![]]
    );
    assert_eq!(bench.keys.layers().active(), 0);
    assert!(
        !bench
            .keys
            .take_notices()
            .iter()
            .any(|notice| matches!(notice, keys::Notice::Brightness { .. }))
    );
}

#[test]
fn a_layer_key_pressed_while_a_macro_plays_stops_it_and_releases_its_keys() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0)), (FN, Action::Layer(Layer::One))]).with(
        0,
        &played(
            Mode::Toggle,
            [press(LETTER_A), Step::Delay(50), release(LETTER_A)],
        ),
    );
    bench.press(KEY_A);
    assert_eq!(bench.passes(2), [vec![], vec![LETTER_A]]);
    bench.release(KEY_A);
    bench.press(FN);
    assert_eq!(bench.pass(TAKEN), Vec::<u8>::new());
    assert_eq!(bench.keys.layers().active(), 1);
    bench.release(FN);
    assert_eq!(bench.passes(3), vec![Vec::<u8>::new(); 3]);
}

#[test]
fn letting_go_of_the_layer_key_a_macro_was_started_on_keeps_it_playing() {
    let mut bench = Bench::new(&[(KEY_D, Action::None), (FN, Action::Layer(Layer::One))]).with(
        0,
        &played(
            Mode::Once,
            [press(LETTER_A), Step::Delay(3), release(LETTER_A)],
        ),
    );
    bench.keys.set_action(KEY_D, Layer::One, Action::Macro(0));
    bench.press(FN);
    bench.pass(TAKEN);
    bench.press(KEY_D);
    assert_eq!(bench.passes(2), [vec![], vec![LETTER_A]]);
    bench.release(FN);
    bench.release(KEY_D);
    assert_eq!(
        bench.passes(4),
        [vec![LETTER_A], vec![LETTER_A], vec![], vec![]]
    );
}

#[test]
fn a_profile_switch_stops_the_macro_and_releases_its_keys() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))])
        .with(0, &played(Mode::Toggle, [press(LETTER_A), Step::Delay(50)]));
    bench.press(KEY_A);
    assert_eq!(bench.passes(2), [vec![], vec![LETTER_A]]);
    let profile = keys::Shared::<NoopRawMutex>::new();
    profile.edit(|_| ());
    bench.keys.configure(&profile, Parts::ACTIVATED);
    assert_eq!(bench.passes(3), vec![Vec::<u8>::new(); 3]);
}

#[test]
fn turning_key_reports_off_stops_the_macro_for_good() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))])
        .with(0, &played(Mode::Toggle, [press(LETTER_A), Step::Delay(50)]));
    bench.press(KEY_A);
    assert_eq!(bench.passes(2), [vec![], vec![LETTER_A]]);
    bench.keys.set_key_reports(false);
    assert_eq!(bench.pass(TAKEN), Vec::<u8>::new());
    bench.keys.set_key_reports(true);
    assert_eq!(bench.passes(3), vec![Vec::<u8>::new(); 3]);
}

#[test]
fn a_stop_from_the_key_task_releases_everything_the_macro_holds() {
    let mut bench = Bench::new(&[(KEY_A, Action::Macro(0))]).with(
        0,
        &played(
            Mode::Once,
            [
                press(LETTER_A),
                Step::Press(Action::Mouse(MouseButton::Left)),
                Step::Delay(50),
            ],
        ),
    );
    bench.press(KEY_A);
    bench.passes(3);
    assert_eq!(bench.keys.output().mouse().buttons, 1);
    assert_eq!(usages(bench.keys.output()), [LETTER_A]);
    bench.keys.stop_macro();
    assert_eq!(bench.pass(TAKEN), Vec::<u8>::new());
    assert_eq!(bench.mouse().buttons, 0);
}

#[test]
fn a_stored_macro_bound_in_the_keymap_plays_through_the_key_task_once_thread_mode_loads_it() {
    let world = World::new();
    let mut settings = storage();
    let model = world.model(&mut settings);
    let mut configurator = Configurator::new(&model, capabilities());
    let mut buffer = [0; 4096];
    let len = wire::encode(&played(Mode::Once, [tap(LETTER_B), text("A")]), &mut buffer).unwrap();
    let value = Encoded::parse(&buffer[..len]).unwrap();
    assert_eq!(configurator.set_macro(2, value), Ok(()));
    let mut keymap = Keymap::new();
    configurator
        .keymap(Slot::new(0), Layer::Base, &mut keymap)
        .unwrap();
    keymap[KEY_A.index(COLUMNS)] = Action::Macro(2);
    configurator
        .set_keymap(Slot::new(0), Layer::Base, &keymap)
        .unwrap();
    let mut bench = Bench80::new(world.task(UsbMode::NoGamepad));
    let loader = Configurator::new(&model, capabilities());
    let mut profiles = pin!(loader.serve());
    let _ = world.keyboard();
    bench.press(KEY_A, FULL);
    let mut sent = Vec::new();
    for _ in 0..10 {
        bench.cycle();
        let _ = poll_once(profiles.as_mut());
        sent.extend(world.keyboard());
    }
    assert_eq!(
        sent,
        [
            vec![LETTER_B],
            vec![],
            vec![SHIFT],
            vec![LETTER_A, SHIFT],
            vec![]
        ]
    );
}
