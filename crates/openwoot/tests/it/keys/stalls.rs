use core::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use embassy_futures::block_on;
use embassy_sync::mutex::Mutex;
use openwoot::{
    keys::{Configurator, Model},
    protocol::Configuration,
    storage::{NorLayout, Partition, Settings},
    usb::Mode,
};
use openwoot_image::Region;
use openwoot_protocol::profile::Slot;

use super::{
    pipeline::{Bench80, FULL, KEYMAP, PROFILE_KEY, World, capabilities},
    stalled::Stalled,
    support::{COLUMNS, KEY_A, ROWS},
};
use crate::{
    scan::Operation,
    storage::{Nor, SECTOR},
};

const LAYOUT: NorLayout = NorLayout {
    capacity: 6 * SECTOR,
    settings: Region {
        start: 0,
        end: 4 * SECTOR,
    },
    device_info: None,
};

fn stalled_storage() -> Option<Settings<Stalled<Nor>>> {
    let formatted = block_on(Settings::open(
        Partition::new(Nor::new(6), LAYOUT.settings_region()),
        false,
    ))
    .ok()?
    .0
    .release()?
    .release();
    block_on(Settings::open(
        Partition::new(Stalled(formatted), LAYOUT.settings_region()),
        false,
    ))
    .ok()
    .map(|(settings, _)| settings)
}

fn poll_once<F: Future>(future: core::pin::Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

#[test]
fn typing_goes_on_while_a_profile_switch_waits_for_a_flash_write_that_never_ends() {
    let world = World::new();
    let model = Mutex::new(block_on(Model::<_, _, ROWS, COLUMNS>::open(
        &world.shared,
        stalled_storage(),
        KEYMAP,
        0,
        4,
    )));
    let mut configurator = Configurator::new(&model, capabilities());
    let mut bench = Bench80::new(world.task(Mode::NoGamepad));
    bench.press(PROFILE_KEY, FULL);
    bench.cycle();
    let requested = block_on(world.shared.profile_request());
    let mut activation = pin!(configurator.activate_profile(Slot::new(requested)));
    assert!(poll_once(activation.as_mut()).is_pending());
    bench.press(PROFILE_KEY, 0);
    bench.cycle();
    for _ in 0..3 {
        let _ = world.reports.take();
        bench.press(KEY_A, FULL);
        bench.cycle();
        assert_eq!(world.keyboard(), Some(vec![0x04]));
        bench.press(KEY_A, 0);
        bench.cycle();
        assert_eq!(world.keyboard(), Some(vec![]));
        bench.press(PROFILE_KEY, FULL);
        bench.cycle();
        bench.press(PROFILE_KEY, 0);
        bench.cycle();
    }
    assert!(poll_once(activation.as_mut()).is_pending());
    assert_eq!(bench.keys.profiles().active(), 0);
}

#[test]
fn typing_goes_on_while_lighting_never_takes_its_events_and_notices() {
    let world = World::new();
    let _model = world.model();
    let mut bench = Bench80::new(world.task(Mode::NoGamepad));
    for _ in 0..40 {
        bench.press(KEY_A, FULL);
        bench.cycle();
        assert_eq!(world.keyboard(), Some(vec![0x04]));
        bench.press(KEY_A, 0);
        bench.cycle();
        assert_eq!(world.keyboard(), Some(vec![]));
    }
    assert!(world.events.is_full());
}

#[test]
fn a_scan_cycle_that_never_ends_is_recovered_and_typing_resumes() {
    let world = World::new();
    let _model = world.model();
    let mut bench = Bench80::new(world.task(Mode::NoGamepad));
    bench.scan.with(|scanner| scanner.prepare(1000));
    assert!(bench.scan.start());
    bench.scan.on_conversion();
    assert!(bench.scan.is_running());
    assert!(bench.scan.recover());
    assert!(!bench.scan.is_running());
    assert_eq!(
        bench
            .scan
            .with(|scanner| scanner.converter().operations.last().copied()),
        Some(Some(Operation::Recover))
    );
    let _ = world.reports.take();
    bench.press(KEY_A, FULL);
    bench.cycle();
    bench.cycle();
    assert_eq!(world.keyboard(), Some(vec![0x04]));
}
