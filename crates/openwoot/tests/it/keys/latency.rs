use openwoot::{keys::Key, usb::Mode};

use super::pipeline::{Bench80, FULL, World, storage};

const KEY: Key = Key::new(3, 1);
const USAGE: u8 = 0x04;

fn settled(world: &World) -> Bench80<'_> {
    let bench = Bench80::new(world.task(Mode::NoGamepad));
    let _ = world.reports.take();
    bench
}

#[test]
fn a_press_before_its_sample_is_reported_at_the_end_of_the_same_scan_cycle() {
    let world = World::new();
    let mut settings = storage();
    let _model = world.model(&mut settings);
    let mut bench = settled(&world);
    bench.press(KEY, FULL);
    bench.cycle();
    assert_eq!(world.keyboard(), Some(vec![USAGE]));
}

#[test]
fn a_press_after_its_sample_is_reported_at_the_end_of_the_next_scan_cycle() {
    let world = World::new();
    let mut settings = storage();
    let _model = world.model(&mut settings);
    let mut bench = settled(&world);
    bench.cycle_with(|bench| bench.press(KEY, FULL));
    assert_eq!(world.keyboard(), None);
    bench.cycle();
    assert_eq!(world.keyboard(), Some(vec![USAGE]));
}
