use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    future::Future,
    pin::{Pin, pin},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

use embassy_futures::{join::join3, yield_now};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, MockDriver};
use embassy_usb_driver::{Event, Speed as BusSpeed};
use openwoot::{
    diagnostics::{Diagnostics, Hardware},
    hid::{
        CHANNEL_DESCRIPTOR, CONTROLS_DESCRIPTOR, ConsumerReport, GAMEPAD_DESCRIPTOR, GamepadReport,
        InputReport, KEY_STATE_DESCRIPTOR, KeyStateReport, MouseReport, NKRO_DESCRIPTOR,
        NkroReport, PLACEHOLDER_DESCRIPTOR, PLACEHOLDER_LEN, SystemReport,
    },
    keys::{Key, Keys, Notice, Output, RapidTrigger, Shared, Switch, Tuning},
    layout::Standard,
    lighting::{HostControl, KeyEvents, LedMatrix},
    profile,
    protocol::{self, Handler},
    runtime::{self, Device, KeyNotices, KeyReports, KeyTask, LedWindow, Progress, Usb},
    scan::{self, Map, Position, Scanner, SensorReadout},
    usb::{Face, HostState, KeyboardFormat, Mode, PollingRate, Resources, Speed},
};
use openwoot_protocol::{
    configuration::Rejection,
    decode_response,
    diagnostics::Usb as UsbReadout,
    encode_request,
    endpoints::{GetGamepadState, GetVersion},
    gamepad::{self, Player},
    keys::{Action, Layer},
    lighting::Effects,
    report,
};

use crate::{
    lighting::{PAGES, START_MS, lighting},
    scan::{Bench, Simulated, sensors},
    support::{
        EXAMPLE_SERIAL, Host, IDENTITY, Outcome, Recorder, hold_clock, parse_configuration, poll,
        setup, stock_identity,
    },
};

const GET_DESCRIPTOR: u8 = 0x06;
const SET_CONFIGURATION: u8 = 0x09;
const SET_FEATURE: u8 = 0x03;
const DEVICE_REMOTE_WAKEUP: u16 = 1;
const DEVICE_TO_HOST: u8 = 0x80;
const INTERFACE_RECIPIENT: u8 = 0x01;
const GAMEPAD_IN: u8 = 0x81;
const KEYBOARD_IN: u8 = 0x82;
const CHANNEL_OUT: u8 = 0x03;
const CHANNEL_IN: u8 = 0x83;
const CONTROLS_IN: u8 = 0x84;
const KEY_STATE_IN: u8 = 0x85;
const CHANNEL_PACKET_SIZE: usize = 64;
const XINPUT_OUT: u8 = 0x01;
const XINPUT_PACKET_SIZE: usize = 32;
const ROWS: usize = 6;
const COLUMNS: usize = 17;
const KEY_A: Key = Key::new(3, 1);
const KEY_MUTE: Key = Key::new(3, 2);
const USAGE_A: u8 = 0x04;
const MUTE: u16 = 0xE2;
const PRESSED: u16 = 2000;
const FULL: u16 = 1400;
const SETTLING_CYCLES: u32 = 6;
const MAX_CONVERSIONS: usize = 100_000;
const SWITCH: Switch = Switch {
    minimum: 112,
    travel_limit: 4000,
};
const TUNING: Tuning = Tuning {
    actuation_point: 1000,
    rapid_trigger_enabled: false,
    rapid_trigger: RapidTrigger {
        press: 200,
        release: 200,
        continuous: false,
    },
    tap_ms: 20,
    keyboard_in_gamepad_mode: false,
};
const NONE: u8 = 0xFF;
const MAP: Map<6, 15> = Map::from_codes([
    [
        0x61, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE,
        NONE,
    ],
    [NONE; 16],
    [NONE; 16],
    [NONE; 16],
    [NONE; 16],
    [NONE; 16],
]);

type Mutual = CriticalSectionRawMutex;

struct Session {
    device: Vec<u8>,
    configuration: Vec<u8>,
    report_descriptors: Vec<Result<Vec<u8>, Outcome>>,
    configured: Option<Outcome>,
    idle_reports: Vec<(u8, Vec<Vec<u8>>)>,
    version_response: Vec<u8>,
    xinput_player: Option<u8>,
}

const HIGH_SPEED: fn() -> Option<BusSpeed> = || Some(BusSpeed::High);
const FULL_SPEED: fn() -> Option<BusSpeed> = || Some(BusSpeed::Full);
const BEFORE_RESET: fn() -> Option<BusSpeed> = || None;

fn connect(
    face: Face,
    negotiated_speed: fn() -> Option<BusSpeed>,
    version_request: &[u8],
    xinput_leds: &[u8],
) -> Result<Session, Outcome> {
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let key_reports = KeyReports::<CriticalSectionRawMutex>::new();
    let usb = Usb::new(
        host.driver(),
        negotiated_speed,
        face,
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut update_mode = Recorder::default();
    let handler = Handler::new(
        protocol::version("1", "2", "3"),
        stock_identity(Some(EXAMPLE_SERIAL)),
    );
    let Usb {
        mut device,
        channel,
        reports,
    } = usb;
    let mut firmware = pin!(join3(
        device.run(),
        channel.run(handler, &mut update_mode),
        reports.run(&key_reports),
    ));
    host.attach(firmware.as_mut());
    let get = |firmware: Pin<&mut _>, request_type: u8, value: u16, index: u16| {
        host.control_in(
            firmware,
            setup(request_type, GET_DESCRIPTOR, value, index, 0xFFFF),
        )
    };
    let device_descriptor = get(firmware.as_mut(), DEVICE_TO_HOST, 0x0100, 0)?;
    let configuration = get(firmware.as_mut(), DEVICE_TO_HOST, 0x0200, 0)?;
    let report_descriptors = (0..5)
        .map(|interface| {
            get(
                firmware.as_mut(),
                DEVICE_TO_HOST | INTERFACE_RECIPIENT,
                0x2200,
                interface,
            )
        })
        .collect();
    let configured = host.control_out(firmware.as_mut(), setup(0, SET_CONFIGURATION, 1, 0, 0), &[]);
    settle(firmware.as_mut());
    let idle_reports = [GAMEPAD_IN, KEYBOARD_IN, CONTROLS_IN, KEY_STATE_IN]
        .into_iter()
        .map(|endpoint| (endpoint, host.take_sent(endpoint)))
        .collect();
    if !version_request.is_empty() {
        host.send_packets(CHANNEL_OUT, version_request, CHANNEL_PACKET_SIZE);
    }
    if !xinput_leds.is_empty() {
        host.send_packets(XINPUT_OUT, xinput_leds, XINPUT_PACKET_SIZE);
    }
    settle(firmware.as_mut());
    Ok(Session {
        device: device_descriptor,
        configuration,
        report_descriptors,
        configured,
        idle_reports,
        version_response: host.take_sent(CHANNEL_IN).concat(),
        xinput_player: state.xinput_player(),
    })
}

fn settle<F: Future>(firmware: Pin<&mut F>) {
    assert!(poll(firmware).is_none());
}

fn gamepad_face(polling: PollingRate) -> Face {
    Face {
        mode: Mode::Gamepad,
        keyboard: KeyboardFormat::NKeyRollover,
        speed: Speed::High,
        gamepad_polling: polling,
    }
}

#[test]
fn the_firmware_enumerates_and_answers_get_version_at_both_speeds() {
    let mut request = [0; report::MAX_LEN];
    let request = encode_request::<GetVersion>(7, &(), &mut request).unwrap();
    for negotiated_speed in [HIGH_SPEED, FULL_SPEED] {
        let session = connect(Face::default(), negotiated_speed, request, &[]).unwrap();
        assert_eq!(session.device[..2], [18, 1]);
        assert_eq!(
            u16::from_le_bytes([session.device[10], session.device[11]]),
            IDENTITY.ids.product_id_base + Mode::NoGamepad.product_id_offset()
        );
        let configuration = parse_configuration(&session.configuration).unwrap();
        assert_eq!(configuration.interface_count, 5);
        let expected = [
            PLACEHOLDER_DESCRIPTOR,
            NKRO_DESCRIPTOR,
            CHANNEL_DESCRIPTOR,
            CONTROLS_DESCRIPTOR,
            KEY_STATE_DESCRIPTOR,
        ];
        for (served, expected) in session.report_descriptors.iter().zip(expected) {
            assert_eq!(served.as_deref(), Ok(expected));
        }
        assert_eq!(session.configured, Some(Outcome::Accepted));
        let version = decode_response::<GetVersion>(7, &session.version_response)
            .unwrap()
            .unwrap();
        assert_eq!(version, protocol::version("1", "2", "3"));
    }
}

#[test]
fn the_gamepad_interval_follows_the_negotiated_speed() {
    for (polling, negotiated_speed, interval) in [
        (PollingRate::Every1Millisecond, HIGH_SPEED, 4),
        (PollingRate::Every1Millisecond, FULL_SPEED, 1),
        (PollingRate::Every4Milliseconds, HIGH_SPEED, 6),
        (PollingRate::Every4Milliseconds, FULL_SPEED, 4),
        (PollingRate::Every125Microseconds, FULL_SPEED, 1),
        (PollingRate::Every4Milliseconds, BEFORE_RESET, 6),
    ] {
        let session = connect(gamepad_face(polling), negotiated_speed, &[], &[]).unwrap();
        let configuration = parse_configuration(&session.configuration).unwrap();
        let gamepad = &configuration.interfaces[0].endpoints[0];
        assert_eq!(gamepad.address, GAMEPAD_IN);
        assert_eq!(gamepad.interval, interval, "{polling:?}");
        assert_eq!(
            session.report_descriptors[0].as_deref(),
            Ok(GAMEPAD_DESCRIPTOR)
        );
        let keyboard = &configuration.interfaces[1].endpoints[0];
        assert_eq!(keyboard.interval, 1);
    }
}

#[test]
fn every_input_endpoint_sends_its_idle_report_once_configured() {
    let session = connect(Face::default(), FULL_SPEED, &[], &[]).unwrap();
    assert_eq!(
        session.idle_reports,
        [
            (GAMEPAD_IN, vec![vec![0; PLACEHOLDER_LEN]]),
            (KEYBOARD_IN, packets(&NkroReport::new(), 28)),
            (
                CONTROLS_IN,
                [
                    SystemReport::default().to_bytes().as_ref().to_vec(),
                    ConsumerReport::default().to_bytes().as_ref().to_vec(),
                    MouseReport::default().to_bytes().as_ref().to_vec(),
                ]
                .to_vec()
            ),
            (KEY_STATE_IN, packets(&KeyStateReport::default(), 64)),
        ]
    );
    let gamepad = connect(
        gamepad_face(PollingRate::Every1Millisecond),
        HIGH_SPEED,
        &[],
        &[],
    )
    .unwrap();
    assert_eq!(
        gamepad.idle_reports[0],
        (GAMEPAD_IN, packets(&GamepadReport::new(), 32))
    );
}

fn packets<Report: InputReport>(report: &Report, packet_size: usize) -> Vec<Vec<u8>> {
    report
        .to_bytes()
        .as_ref()
        .chunks(packet_size)
        .map(<[u8]>::to_vec)
        .collect()
}

#[test]
fn the_xinput_led_pattern_from_the_host_sets_the_player() {
    let xinput = Face {
        mode: Mode::Xinput,
        ..Face::default()
    };
    let session = connect(xinput, HIGH_SPEED, &[], &[0x01, 0x03, 0x07]).unwrap();
    assert_eq!(session.xinput_player, Some(1));
    let unknown = connect(xinput, HIGH_SPEED, &[], &[0x01, 0x03, 0x00]).unwrap();
    assert_eq!(unknown.xinput_player, None);
}

#[test]
fn the_player_the_host_lights_on_the_xinput_controller_reaches_the_gamepad_state() {
    static STATE: HostState = HostState::new();
    let host = Host::default();
    let mut resources = Resources::new();
    let key_reports = KeyReports::<Mutual>::new();
    let xinput = Face {
        mode: Mode::Xinput,
        ..Face::default()
    };
    let Usb {
        mut device,
        channel,
        reports,
    } = Usb::new(
        host.driver(),
        HIGH_SPEED,
        xinput,
        IDENTITY,
        &mut resources,
        &STATE,
    );
    let handler =
        Handler::new(protocol::version("1", "2", "3"), stock_identity(None)).with_host(&STATE);
    let mut update_mode = Recorder::default();
    let mut firmware = pin!(join3(
        device.run(),
        channel.run(handler, &mut update_mode),
        reports.run(&key_reports),
    ));
    let mut wire = Wire::configured(&host, firmware.as_mut());
    assert_eq!(
        gamepad_state(&mut wire, 1),
        Ok(Some(Ok(gamepad::State::default())))
    );
    host.send_packets(XINPUT_OUT, &[0x01, 0x03, 0x08], XINPUT_PACKET_SIZE);
    wire.settle();
    assert_eq!(
        gamepad_state(&mut wire, 2),
        Ok(Some(Ok(gamepad::State {
            player: Some(Player::Three),
            detection: false,
        })))
    );
    assert_eq!(
        wire.request(setup(0, SET_CONFIGURATION, 1, 0, 0)),
        Some(Outcome::Accepted)
    );
    wire.settle();
    assert_eq!(
        gamepad_state(&mut wire, 3),
        Ok(Some(Ok(gamepad::State::default())))
    );
}

fn gamepad_state<F: Future>(
    wire: &mut Wire<'_, F>,
    sequence: u16,
) -> Result<Option<Result<gamepad::State, Rejection>>, openwoot_protocol::Error> {
    let mut request = [0; report::MAX_LEN];
    let request = encode_request::<GetGamepadState>(sequence, &(), &mut request)?;
    wire.host
        .send_packets(CHANNEL_OUT, request, CHANNEL_PACKET_SIZE);
    wire.settle();
    decode_response::<GetGamepadState>(sequence, &wire.host.take_sent(CHANNEL_IN).concat())
}

struct Wire<'w, F: Future> {
    host: &'w Host,
    firmware: Pin<&'w mut F>,
}

impl<'w, F: Future> Wire<'w, F> {
    fn configured(host: &'w Host, mut firmware: Pin<&'w mut F>) -> Self {
        host.attach(firmware.as_mut());
        assert_eq!(
            host.control_out(firmware.as_mut(), setup(0, SET_CONFIGURATION, 1, 0, 0), &[]),
            Some(Outcome::Accepted)
        );
        let mut wire = Self { host, firmware };
        wire.settle();
        wire
    }

    fn settle(&mut self) {
        settle(self.firmware.as_mut());
    }

    fn request(&mut self, request: [u8; 8]) -> Option<Outcome> {
        self.host.control_out(self.firmware.as_mut(), request, &[])
    }

    fn bus(&mut self, events: &[Event]) {
        self.host.bus_events(self.firmware.as_mut(), events);
    }
}

fn firmware<'a>(
    host: &Host,
    face: Face,
    state: &'a HostState,
    key_reports: &'a KeyReports<Mutual>,
    resources: &'a mut Resources<'a>,
    update_mode: &'a mut Recorder,
) -> impl Future + 'a {
    let Usb {
        device,
        channel,
        reports,
    } = Usb::new(host.driver(), FULL_SPEED, face, IDENTITY, resources, state);
    let mut device = Device::new(device, key_reports);
    let handler = Handler::new(
        protocol::version("1", "2", "3"),
        stock_identity(Some(EXAMPLE_SERIAL)),
    );
    async move {
        join3(
            device.run(),
            channel.run(handler, update_mode),
            reports.run(key_reports),
        )
        .await
    }
}

struct Typist {
    keys: Keys<Standard, ROWS, COLUMNS>,
    travels: HashMap<Key, u16>,
}

impl Typist {
    fn new() -> Self {
        let mut keys = Keys::new(Standard);
        keys.set_tuning(TUNING);
        keys.set_key_reports(true);
        for (key, action) in [
            (KEY_A, Action::Keyboard(USAGE_A)),
            (KEY_MUTE, Action::Consumer(MUTE)),
        ] {
            keys.set_switch(key, SWITCH);
            keys.set_action(key, Layer::Base, action);
        }
        Self {
            keys,
            travels: HashMap::new(),
        }
    }

    fn hit(&mut self, key: Key, down: bool) -> &Output {
        let travel = if down { PRESSED } else { 0 };
        self.travels.insert(key, travel);
        let travels = &self.travels;
        self.keys.pass([(key, travel)], 1, |key| {
            travels.get(&key).copied().unwrap_or_default()
        })
    }
}

fn nkro(usages: &[u8]) -> Vec<u8> {
    let mut report = NkroReport::new();
    for &usage in usages {
        assert!(report.set(usage, true));
    }
    report.to_bytes().to_vec()
}

fn consumer(usage: u16) -> Vec<u8> {
    ConsumerReport {
        usages: [usage, 0, 0],
    }
    .to_bytes()
    .to_vec()
}

struct Peers {
    shared: Shared<Mutual>,
    events: KeyEvents,
    notices: KeyNotices,
    sensors: SensorReadout,
    diagnostics: Diagnostics,
    led_window: LedWindow,
}

impl Peers {
    fn new() -> Self {
        Self {
            shared: Shared::new(),
            events: KeyEvents::new(),
            notices: KeyNotices::new(),
            sensors: SensorReadout::new(),
            diagnostics: Diagnostics::new(),
            led_window: LedWindow::new(),
        }
    }

    fn task<'a>(
        &'a self,
        host: &'a HostState,
        key_reports: &'a KeyReports<Mutual>,
    ) -> KeyTask<'a, Mutual> {
        KeyTask {
            shared: &self.shared,
            reports: key_reports,
            events: &self.events,
            notices: &self.notices,
            host,
            sensors: &self.sensors,
            face: Face::default(),
            diagnostics: &self.diagnostics,
            led_window: &self.led_window,
        }
    }

    fn published(&self) -> u32 {
        self.diagnostics
            .report(UsbReadout::default(), Hardware::default())
            .keys
            .reports
    }

    fn cycles(&self) -> u32 {
        self.diagnostics
            .report(UsbReadout::default(), Hardware::default())
            .scan
            .cycles
    }
}

#[derive(Default)]
struct Wakes(AtomicUsize);

impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

fn finish_scan(scan: &scan::Shared<Bench, 6, 15>) {
    let finished = (0..MAX_CONVERSIONS).find(|_| {
        scan.on_conversion();
        !scan.is_running()
    });
    assert!(finished.is_some(), "the scan never finished");
}

struct Matrix<'a> {
    scan: scan::Shared<Bench, 6, 15>,
    task: KeyTask<'a, Mutual>,
    keys: Keys<Standard, ROWS, COLUMNS>,
    progress: Progress,
    now_ms: u32,
}

impl<'a> Matrix<'a> {
    fn new(task: KeyTask<'a, Mutual>) -> Self {
        let scan = scan::Shared::new();
        assert!(scan.install(|| Scanner::new(
            Simulated::default(),
            MAP,
            [0, 1, 2, 3, 4, 5],
            sensors()
        )));
        let mut keys = Keys::new(Standard);
        keys.set_mode(task.face.mode);
        keys.set_tuning(TUNING);
        keys.set_action(KEY_A, Layer::Base, Action::Keyboard(USAGE_A));
        let mut matrix = Self {
            scan,
            task,
            keys,
            progress: Progress::default(),
            now_ms: 0,
        };
        matrix.scan.with(Scanner::start_sweep);
        matrix.convert();
        for _ in 0..SETTLING_CYCLES {
            matrix.cycle();
        }
        matrix
    }

    fn convert(&self) {
        assert!(self.scan.start());
        finish_scan(&self.scan);
    }

    fn cycle(&mut self) {
        self.now_ms += 1;
        let now_ms = self.now_ms;
        self.scan.with(|scanner| scanner.prepare(now_ms));
        self.convert();
        if self
            .task
            .pass(&self.scan, &mut self.keys, &mut self.progress, now_ms)
        {
            self.task
                .deliver(&mut self.keys, &mut self.progress, now_ms);
        }
    }

    fn press(&self, key: Key, magnitude: u16) {
        let input = MAP.position(key).map_or(0, Position::input);
        self.scan
            .with(|scanner| scanner.converter_mut().press(input, 0, magnitude));
    }
}

#[test]
fn a_host_that_reads_only_the_keyboard_endpoint_gets_every_press_and_release() {
    let host = Host::default();
    let state = HostState::new();
    let key_reports = KeyReports::new();
    let mut resources = Resources::new();
    let mut update_mode = Recorder::default();
    for endpoint in [GAMEPAD_IN, CONTROLS_IN, KEY_STATE_IN] {
        host.stop_reading(endpoint);
    }
    let firmware = pin!(firmware(
        &host,
        Face::default(),
        &state,
        &key_reports,
        &mut resources,
        &mut update_mode
    ));
    let mut wire = Wire::configured(&host, firmware);
    let mut typist = Typist::new();
    key_reports.publish(typist.keys.output());
    wire.settle();
    assert_eq!(host.take_sent(KEYBOARD_IN), [nkro(&[]), nkro(&[])]);
    for (key, down) in [(KEY_MUTE, true), (KEY_MUTE, false)] {
        key_reports.publish(typist.hit(key, down));
        wire.settle();
    }
    for down in [true, false, true, false] {
        key_reports.publish(typist.hit(KEY_A, down));
        wire.settle();
    }
    assert_eq!(
        host.take_sent(KEYBOARD_IN),
        [nkro(&[USAGE_A]), nkro(&[]), nkro(&[USAGE_A]), nkro(&[])]
    );
    assert_eq!(host.sent_len(KEY_STATE_IN), 0);
    assert_eq!(host.sent_len(CONTROLS_IN), 0);
}

#[test]
fn a_key_state_reader_that_goes_away_mid_press_holds_back_no_keystroke() {
    let host = Host::default();
    let state = HostState::new();
    let key_reports = KeyReports::new();
    let mut resources = Resources::new();
    let mut update_mode = Recorder::default();
    let firmware = pin!(firmware(
        &host,
        Face::default(),
        &state,
        &key_reports,
        &mut resources,
        &mut update_mode
    ));
    let mut wire = Wire::configured(&host, firmware);
    let _ = host.take_sent(KEYBOARD_IN);
    let mut typist = Typist::new();
    key_reports.publish(typist.hit(KEY_A, true));
    wire.settle();
    assert!(host.sent_len(KEY_STATE_IN) > 0);
    host.stop_reading(KEY_STATE_IN);
    host.stop_reading(GAMEPAD_IN);
    for down in [false, true, false] {
        key_reports.publish(typist.hit(KEY_A, down));
        wire.settle();
    }
    for down in [true, false] {
        key_reports.publish(typist.hit(KEY_MUTE, down));
        wire.settle();
    }
    assert_eq!(
        host.take_sent(KEYBOARD_IN),
        [nkro(&[USAGE_A]), nkro(&[]), nkro(&[USAGE_A]), nkro(&[])]
    );
    let controls = host.take_sent(CONTROLS_IN);
    assert_eq!(
        controls.get(controls.len() - 2..),
        Some(&[consumer(MUTE), consumer(0)][..])
    );
}

#[test]
fn two_taps_inside_one_slow_host_poll_both_reach_the_host_in_order() {
    let host = Host::default();
    let state = HostState::new();
    let key_reports = KeyReports::new();
    let mut resources = Resources::new();
    let mut update_mode = Recorder::default();
    let firmware = pin!(firmware(
        &host,
        Face::default(),
        &state,
        &key_reports,
        &mut resources,
        &mut update_mode
    ));
    let mut wire = Wire::configured(&host, firmware);
    let _ = host.take_sent(KEYBOARD_IN);
    host.stop_reading(KEYBOARD_IN);
    let mut typist = Typist::new();
    for down in [true, false, true, false] {
        key_reports.publish(typist.hit(KEY_A, down));
        wire.settle();
    }
    assert_eq!(host.sent_len(KEYBOARD_IN), 0);
    host.resume_reading(KEYBOARD_IN);
    wire.settle();
    assert_eq!(
        host.take_sent(KEYBOARD_IN),
        [nkro(&[USAGE_A]), nkro(&[]), nkro(&[USAGE_A]), nkro(&[])]
    );
}

#[test]
fn a_reconfigured_device_sends_the_idle_report_then_the_key_still_held() {
    let host = Host::default();
    let state = HostState::new();
    let key_reports = KeyReports::new();
    let mut resources = Resources::new();
    let mut update_mode = Recorder::default();
    let firmware = pin!(firmware(
        &host,
        Face::default(),
        &state,
        &key_reports,
        &mut resources,
        &mut update_mode
    ));
    let mut wire = Wire::configured(&host, firmware);
    let mut typist = Typist::new();
    key_reports.publish(typist.hit(KEY_A, true));
    wire.settle();
    assert_eq!(host.take_sent(KEYBOARD_IN), [nkro(&[]), nkro(&[USAGE_A])]);
    assert_eq!(
        wire.request(setup(0, SET_CONFIGURATION, 1, 0, 0)),
        Some(Outcome::Accepted)
    );
    wire.settle();
    assert!(key_reports.publish(typist.keys.output()));
    wire.settle();
    assert_eq!(host.take_sent(KEYBOARD_IN), [nkro(&[]), nkro(&[USAGE_A])]);
}

#[test]
fn a_key_press_wakes_a_suspended_host_that_armed_remote_wake_up() {
    let host = Host::default();
    let state = HostState::new();
    let key_reports = KeyReports::new();
    let peers = Peers::new();
    let mut resources = Resources::new();
    let mut update_mode = Recorder::default();
    let firmware = pin!(firmware(
        &host,
        Face::default(),
        &state,
        &key_reports,
        &mut resources,
        &mut update_mode
    ));
    let mut wire = Wire::configured(&host, firmware);
    assert_eq!(
        wire.request(setup(0, SET_FEATURE, DEVICE_REMOTE_WAKEUP, 0, 0)),
        Some(Outcome::Accepted)
    );
    let mut matrix = Matrix::new(peers.task(&state, &key_reports));
    wire.settle();
    let _ = host.take_sent(KEYBOARD_IN);
    wire.bus(&[Event::Suspend]);
    assert!(state.is_suspended());
    let published = peers.published();
    matrix.press(KEY_A, FULL);
    matrix.cycle();
    assert_eq!(peers.published(), published);
    wire.settle();
    assert!(!state.is_suspended());
    assert_eq!(host.sent_len(KEYBOARD_IN), 0);
    matrix.cycle();
    wire.settle();
    assert_eq!(host.take_sent(KEYBOARD_IN), [nkro(&[USAGE_A])]);
}

#[test]
fn a_tap_while_the_host_sleeps_unarmed_is_not_replayed_after_resume() {
    let host = Host::default();
    let state = HostState::new();
    let key_reports = KeyReports::new();
    let peers = Peers::new();
    let mut resources = Resources::new();
    let mut update_mode = Recorder::default();
    let firmware = pin!(firmware(
        &host,
        Face::default(),
        &state,
        &key_reports,
        &mut resources,
        &mut update_mode
    ));
    let mut wire = Wire::configured(&host, firmware);
    let mut matrix = Matrix::new(peers.task(&state, &key_reports));
    wire.settle();
    let _ = host.take_sent(KEYBOARD_IN);
    wire.bus(&[Event::Suspend]);
    matrix.press(KEY_A, FULL);
    matrix.cycle();
    wire.settle();
    assert!(state.is_suspended());
    matrix.press(KEY_A, 0);
    matrix.cycle();
    wire.settle();
    assert!(state.is_suspended());
    assert_eq!(host.sent_len(KEYBOARD_IN), 0);
    wire.bus(&[Event::Resume]);
    assert!(!state.is_suspended());
    matrix.cycle();
    wire.settle();
    assert_eq!(host.take_sent(KEYBOARD_IN), [nkro(&[])]);
}

#[test]
fn a_brightness_tap_shorter_than_the_repeat_delay_steps_once() {
    let notices = KeyNotices::new();
    let mut lights = lighting(255, Effects::new());
    assert!(notices.send(Notice::Brightness {
        increase: false,
        held: true,
    }));
    assert!(notices.send(Notice::Brightness {
        increase: false,
        held: false,
    }));
    for now_ms in 0..200 {
        notices.apply(&mut lights, now_ms);
    }
    assert_eq!(lights.brightness(), 250);
}

#[test]
fn a_held_brightness_key_steps_at_once_and_then_every_50_ms() {
    let notices = KeyNotices::new();
    let mut lights = lighting(255, Effects::new());
    notices.send(Notice::Brightness {
        increase: false,
        held: true,
    });
    notices.apply(&mut lights, 0);
    assert_eq!(lights.brightness(), 250);
    notices.apply(&mut lights, 49);
    assert_eq!(lights.brightness(), 250);
    notices.apply(&mut lights, 50);
    assert_eq!(lights.brightness(), 245);
    notices.apply(&mut lights, 100);
    assert_eq!(lights.brightness(), 240);
    notices.send(Notice::Brightness {
        increase: false,
        held: false,
    });
    for now_ms in 101..400 {
        notices.apply(&mut lights, now_ms);
    }
    assert_eq!(lights.brightness(), 240);
}

#[test]
fn a_brightness_release_behind_a_full_notice_queue_still_stops_the_repeat() {
    let notices = KeyNotices::new();
    let mut lights = lighting(255, Effects::new());
    notices.send(Notice::Brightness {
        increase: false,
        held: true,
    });
    notices.apply(&mut lights, 0);
    let queued = (0..1000)
        .take_while(|_| notices.send(Notice::ShowLayer(0)))
        .count();
    assert!(queued < 1000);
    assert!(notices.send(Notice::Brightness {
        increase: false,
        held: false,
    }));
    for now_ms in 1..4000 {
        notices.apply(&mut lights, now_ms);
    }
    assert_eq!(lights.brightness(), 250);
}

#[test]
fn the_key_task_waits_on_a_timer_for_its_scanner_and_recovers_stalled_scans() {
    let _clock = hold_clock();
    let clock = MockDriver::get();
    let host = HostState::new();
    let key_reports = KeyReports::new();
    let peers = Peers::new();
    let task = peers.task(&host, &key_reports);
    let scan = scan::Shared::<Bench, 6, 15>::new();
    let mut keys = Keys::<Standard, ROWS, COLUMNS>::new(Standard);
    let wake_count = Arc::new(Wakes::default());
    let waker = Waker::from(Arc::clone(&wake_count));
    let mut context = Context::from_waker(&waker);
    let scan_counts = || {
        let scan = peers
            .diagnostics
            .report(UsbReadout::default(), Hardware::default())
            .scan;
        (scan.cycles, scan.recoveries)
    };
    let mut run = pin!(task.run(&scan, &mut keys));
    assert!(run.as_mut().poll(&mut context).is_pending());
    assert_eq!(wake_count.0.load(Ordering::Relaxed), 0);
    clock.advance(Duration::from_millis(1));
    assert_eq!(wake_count.0.load(Ordering::Relaxed), 1);
    assert!(scan.install(|| Scanner::new(
        Simulated::default(),
        MAP,
        [0, 1, 2, 3, 4, 5],
        sensors()
    )));
    assert!(run.as_mut().poll(&mut context).is_pending());
    assert!(scan.is_running());
    clock.advance(Duration::from_secs(1));
    assert!(run.as_mut().poll(&mut context).is_pending());
    assert_eq!(scan_counts(), (0, 1));
    finish_scan(&scan);
    assert!(run.as_mut().poll(&mut context).is_pending());
    finish_scan(&scan);
    assert!(run.as_mut().poll(&mut context).is_pending());
    assert_eq!(scan_counts(), (1, 1));
    assert!(scan.is_running());
    clock.advance(Duration::from_millis(50));
    assert!(run.as_mut().poll(&mut context).is_pending());
    assert_eq!(scan_counts(), (1, 2));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TransferStart {
    scanning: bool,
    cycles: u32,
}

#[derive(Default)]
struct LedBus {
    busy: Cell<bool>,
    starts: RefCell<Vec<TransferStart>>,
}

struct SlowLeds<'a> {
    bus: &'a LedBus,
    scan: &'a scan::Shared<Bench, 6, 15>,
    peers: &'a Peers,
    polls: usize,
}

impl SlowLeds<'_> {
    async fn transfer(&self) -> Result<(), ()> {
        self.bus.starts.borrow_mut().push(TransferStart {
            scanning: self.scan.is_running(),
            cycles: self.peers.cycles(),
        });
        self.bus.busy.set(true);
        for _ in 0..self.polls {
            yield_now().await;
        }
        self.bus.busy.set(false);
        Ok(())
    }
}

impl LedMatrix for SlowLeds<'_> {
    type Error = ();

    async fn initialize(&mut self) -> Result<(), ()> {
        self.transfer().await
    }

    async fn set_running(&mut self, _running: bool) -> Result<(), ()> {
        self.transfer().await
    }

    async fn write(&mut self, _bank: u8, _first_register: u8, _values: &[u8]) -> Result<(), ()> {
        self.transfer().await
    }
}

fn installed_scan() -> scan::Shared<Bench, 6, 15> {
    let scan = scan::Shared::new();
    assert!(scan.install(|| Scanner::new(
        Simulated::default(),
        MAP,
        [0, 1, 2, 3, 4, 5],
        sensors()
    )));
    scan
}

fn transfers_beside_the_scan(polls_per_transfer: usize) -> Vec<TransferStart> {
    let _clock = hold_clock();
    let host = HostState::new();
    let key_reports = KeyReports::new();
    let peers = Peers::new();
    let task = peers.task(&host, &key_reports);
    let scan = installed_scan();
    let mut keys = Keys::<Standard, ROWS, COLUMNS>::new(Standard);
    let bus = LedBus::default();
    let mut leds = SlowLeds {
        bus: &bus,
        scan: &scan,
        peers: &peers,
        polls: polls_per_transfer,
    };
    let mut engine = lighting(255, Effects::new());
    let mut lights = runtime::Lighting {
        lighting: &mut engine,
        events: &KeyEvents::new(),
        notices: &KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &Diagnostics::new(),
        profile: &profile::Shared::<Mutual>::new(),
        led_window: &peers.led_window,
    };
    let mut context = Context::from_waker(Waker::noop());
    let mut run = pin!(task.run(&scan, &mut keys));
    let mut step = pin!(lights.step(&mut leds, &(), START_MS, true));
    let mut delivered = None;
    for _ in 0..MAX_CONVERSIONS {
        if let Poll::Ready(outcome) = step.as_mut().poll(&mut context) {
            delivered = Some(outcome);
            break;
        }
        assert!(run.as_mut().poll(&mut context).is_pending());
        assert!(
            !(bus.busy.get() && scan.is_running()),
            "a scan cycle started during an LED transfer"
        );
        if scan.is_running() {
            scan.on_conversion();
        }
    }
    assert_eq!(delivered, Some(true), "the LED transfers never finished");
    bus.starts.take()
}

#[test]
fn no_conversion_runs_while_an_led_transfer_is_on_the_bus() {
    let starts = transfers_beside_the_scan(7);
    assert_eq!(starts.len(), 1 + PAGES, "{starts:?}");
    assert!(
        starts.iter().all(|start| !start.scanning),
        "an LED transfer started during a scan cycle: {starts:?}"
    );
}

#[test]
fn the_key_task_lets_one_led_page_out_between_two_scan_cycles() {
    let starts = transfers_beside_the_scan(1);
    let cycles: Vec<u32> = starts.iter().map(|start| start.cycles).collect();
    let expected: Vec<u32> = (1..=u32::try_from(1 + PAGES).unwrap()).collect();
    assert_eq!(
        cycles, expected,
        "the wake-up waits for the calibration and the first cycle, then one transfer follows each cycle"
    );
}

#[test]
fn an_led_transfer_that_never_ends_holds_the_scan_back_for_the_transfer_limit_only() {
    let _clock = hold_clock();
    let clock = MockDriver::get();
    let host = HostState::new();
    let key_reports = KeyReports::new();
    let peers = Peers::new();
    let task = peers.task(&host, &key_reports);
    let scan = installed_scan();
    let mut keys = Keys::<Standard, ROWS, COLUMNS>::new(Standard);
    let bus = LedBus::default();
    let mut leds = SlowLeds {
        bus: &bus,
        scan: &scan,
        peers: &peers,
        polls: usize::MAX,
    };
    let mut engine = lighting(255, Effects::new());
    let mut lights = runtime::Lighting {
        lighting: &mut engine,
        events: &KeyEvents::new(),
        notices: &KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &Diagnostics::new(),
        profile: &profile::Shared::<Mutual>::new(),
        led_window: &peers.led_window,
    };
    let mut context = Context::from_waker(Waker::noop());
    let mut run = pin!(task.run(&scan, &mut keys));
    let mut step = pin!(lights.step(&mut leds, &(), START_MS, true));
    for _ in 0..MAX_CONVERSIONS {
        assert!(step.as_mut().poll(&mut context).is_pending());
        if bus.busy.get() {
            break;
        }
        assert!(run.as_mut().poll(&mut context).is_pending());
        if scan.is_running() {
            scan.on_conversion();
        }
    }
    assert!(bus.busy.get(), "the LED transfer never started");
    for _ in 0..10 {
        assert!(step.as_mut().poll(&mut context).is_pending());
        assert!(run.as_mut().poll(&mut context).is_pending());
    }
    assert!(!scan.is_running());
    clock.advance(LedWindow::TRANSFER_LIMIT - Duration::from_ticks(1));
    assert!(run.as_mut().poll(&mut context).is_pending());
    assert!(!scan.is_running(), "the scan resumed before the limit");
    clock.advance(Duration::from_ticks(1));
    assert!(run.as_mut().poll(&mut context).is_pending());
    assert!(scan.is_running(), "the scan still waits for the LED bus");
    let cycles = peers.cycles();
    for cycle in 1..=3 {
        finish_scan(&scan);
        assert!(step.as_mut().poll(&mut context).is_pending());
        assert!(run.as_mut().poll(&mut context).is_pending());
        assert!(
            scan.is_running(),
            "the scan waited again for the same transfer"
        );
        assert_eq!(peers.cycles(), cycles + cycle);
    }
    assert_eq!(bus.starts.borrow().len(), 1);
}

#[test]
fn the_six_input_scan_leaves_the_cpu_idle_for_a_while_every_millisecond() {
    let _clock = hold_clock();
    let clock = MockDriver::get();
    let host = HostState::new();
    let key_reports = KeyReports::new();
    let peers = Peers::new();
    let task = peers.task(&host, &key_reports);
    let scan = installed_scan();
    scan.with(|scanner| scanner.request_mode(scan::Mode::Parallel));
    let mut keys = Keys::<Standard, ROWS, COLUMNS>::new(Standard);
    let mut context = Context::from_waker(Waker::noop());
    let mut run = pin!(task.run(&scan, &mut keys));
    assert!(run.as_mut().poll(&mut context).is_pending());
    finish_scan(&scan);
    assert!(run.as_mut().poll(&mut context).is_pending());
    for _ in 0..5 {
        finish_scan(&scan);
        assert!(run.as_mut().poll(&mut context).is_pending());
        assert!(
            scan.is_running(),
            "the scan paused before a millisecond passed"
        );
    }
    clock.advance(KeyTask::<Mutual>::IDLE_EVERY);
    finish_scan(&scan);
    assert!(run.as_mut().poll(&mut context).is_pending());
    assert!(
        !scan.is_running(),
        "the scan kept the CPU for over a millisecond"
    );
    clock.advance(KeyTask::<Mutual>::IDLE_FOR - Duration::from_ticks(1));
    assert!(run.as_mut().poll(&mut context).is_pending());
    assert!(!scan.is_running(), "the idle slice ended early");
    clock.advance(Duration::from_ticks(1));
    assert!(run.as_mut().poll(&mut context).is_pending());
    assert!(
        scan.is_running(),
        "the scan did not resume after the idle slice"
    );
    assert_eq!(peers.cycles(), 6);
}

#[test]
fn the_single_input_scan_never_pauses_for_idle_time() {
    let _clock = hold_clock();
    let clock = MockDriver::get();
    let host = HostState::new();
    let key_reports = KeyReports::new();
    let peers = Peers::new();
    let task = peers.task(&host, &key_reports);
    let scan = installed_scan();
    let mut keys = Keys::<Standard, ROWS, COLUMNS>::new(Standard);
    let mut context = Context::from_waker(Waker::noop());
    let mut run = pin!(task.run(&scan, &mut keys));
    assert!(run.as_mut().poll(&mut context).is_pending());
    finish_scan(&scan);
    assert!(run.as_mut().poll(&mut context).is_pending());
    for _ in 0..5 {
        clock.advance(KeyTask::<Mutual>::IDLE_EVERY);
        finish_scan(&scan);
        assert!(run.as_mut().poll(&mut context).is_pending());
        assert!(scan.is_running(), "the single-input scan paused");
    }
}
