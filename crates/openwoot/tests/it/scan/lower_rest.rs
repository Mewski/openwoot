use openwoot::{
    analog::{NoiseState, NoiseStatistics},
    scan::{Mode, Position, Sensor, Travel},
};

use super::support::{Bench80, SETTLING_CYCLES, cycle, rest_everywhere, scanner, settled, wobble};

const HELD: Position = Position::new(5, 3);
const CONTROL: Position = Position::new(5, 4);
const SETTLE_CYCLES: usize = 64;
const HELD_LEVEL: u16 = 1300;
const HELD_REST: u16 = 1302;

struct Run {
    scanner: Bench80,
    travel: Travel<6, 15>,
    now_ms: u32,
}

impl Run {
    fn held_at_power_on() -> Self {
        let mut scanner = scanner();
        scanner.converter_mut().noise = Some(wobble);
        rest_everywhere(&mut scanner, 300);
        scanner
            .converter_mut()
            .press(HELD.input(), HELD.address(), HELD_LEVEL);
        let travel = Travel::new();
        let now_ms = settled(&mut scanner, &travel);
        Self {
            scanner,
            travel,
            now_ms,
        }
    }

    fn step(&mut self, readings: &[(Position, u16)]) -> Vec<(Position, u16)> {
        for &(position, magnitude) in readings {
            self.scanner
                .converter_mut()
                .press(position.input(), position.address(), magnitude);
        }
        let sample_us = self.scanner.converter().now_us.wrapping_add(600);
        self.scanner.converter_mut().now_us = sample_us;
        cycle(&mut self.scanner, &self.travel, self.now_ms);
        self.now_ms += 1;
        self.scanner.converter_mut().operations.clear();
        self.scanner
            .events()
            .iter()
            .map(|event| (event.position, event.travel))
            .collect()
    }

    fn hold(&mut self, position: Position, magnitude: u16, cycles: usize) {
        for _ in 0..cycles {
            self.step(&[(position, magnitude)]);
        }
    }

    fn deep_strokes(&mut self, positions: &[Position]) {
        let stroke = [(HELD_LEVEL, 30), (1400, 1), (1500, 41)];
        for (magnitude, cycles) in stroke.repeat(3).into_iter().chain([(HELD_LEVEL, 30)]) {
            let readings: Vec<_> = positions
                .iter()
                .map(|&position| (position, magnitude))
                .collect();
            for _ in 0..cycles {
                self.step(&readings);
            }
        }
    }

    fn rest_level(&self, position: Position) -> u16 {
        self.scanner.sensor(position).map_or(0, Sensor::rest_level)
    }

    fn bottom_level(&self, position: Position) -> u16 {
        self.scanner
            .sensor(position)
            .map_or(0, Sensor::bottom_level)
    }
}

fn press_profile() -> Vec<u16> {
    let mut profile = vec![450, 600, 700, 700, 700, 850, 1000, 1100];
    profile.extend([1200; 20]);
    profile.extend([900, 600, 400]);
    profile.extend([300; 10]);
    profile
}

#[test]
fn a_key_held_at_power_on_reads_its_full_travel_on_the_press_after_its_release() {
    let mut run = Run::held_at_power_on();
    assert_eq!(run.rest_level(HELD), HELD_REST);
    assert_eq!(run.bottom_level(HELD), HELD_REST + 50);
    assert_eq!(run.rest_level(CONTROL), 302);
    run.deep_strokes(&[HELD]);

    let mut lowered_after = None;
    for released in 0..SETTLE_CYCLES + 8 {
        run.step(&[(HELD, 300)]);
        if run.rest_level(HELD) != HELD_REST {
            lowered_after = Some(released + 1);
            break;
        }
    }
    let lowered_after = lowered_after.unwrap();
    assert!(
        (SETTLE_CYCLES + 1..=SETTLE_CYCLES + 4).contains(&lowered_after),
        "lowered after {lowered_after} cycles"
    );
    let held = run.scanner.sensor(HELD).unwrap();
    assert!(held.rest_level().abs_diff(run.rest_level(CONTROL)) <= held.single().noise_band);
    assert_eq!(held.single().rest_level, held.rest_level());
    assert_eq!(held.bottom_level(), 1200);
    assert_eq!(held.noise().state(), NoiseState::Idle);
    assert_eq!(run.travel.get(HELD), 0);

    run.scanner.converter_mut().noise = None;
    let mut held_travel = Vec::new();
    let mut control_travel = Vec::new();
    for magnitude in (0..5).flat_map(|_| press_profile()) {
        for (position, travel) in run.step(&[(HELD, magnitude), (CONTROL, magnitude)]) {
            if position == HELD {
                held_travel.push(travel);
            } else if position == CONTROL {
                control_travel.push(travel);
            }
        }
    }
    assert_eq!(held_travel.len(), control_travel.len());
    for (held, control) in held_travel.iter().zip(&control_travel) {
        assert!(
            held.abs_diff(*control) <= 40,
            "{held_travel:?} {control_travel:?}"
        );
    }
    assert!(
        held_travel
            .iter()
            .any(|&travel| (2500..3500).contains(&travel))
    );
    assert!(held_travel.iter().any(|&travel| travel > 4000));
    assert_eq!(held_travel.last(), Some(&0));
    assert_eq!(run.bottom_level(HELD), run.bottom_level(CONTROL));
}

#[test]
fn a_dropout_or_a_bounce_below_rest_leaves_the_rest_level_alone() {
    let mut run = Run::held_at_power_on();
    run.hold(HELD, 300, 1);
    run.hold(HELD, HELD_LEVEL, 200);
    assert_eq!(run.rest_level(HELD), HELD_REST);

    for _ in 0..40 {
        run.hold(HELD, 300, 3);
        run.hold(HELD, 700, 2);
    }
    run.hold(HELD, HELD_LEVEL, 200);
    assert_eq!(run.rest_level(HELD), HELD_REST);

    run.hold(HELD, 300, SETTLE_CYCLES - 4);
    run.hold(HELD, HELD_LEVEL, 200);
    assert_eq!(run.rest_level(HELD), HELD_REST);

    run.hold(CONTROL, 0, 1);
    run.hold(CONTROL, 300, 200);
    for _ in 0..40 {
        run.hold(CONTROL, 100, 2);
        run.hold(CONTROL, 200, 2);
    }
    run.hold(CONTROL, 300, 200);
    assert_eq!(run.rest_level(CONTROL), 302);
    assert_eq!(run.bottom_level(CONTROL), 1200);

    run.hold(HELD, 300, SETTLE_CYCLES + 4);
    assert!((299..=303).contains(&run.rest_level(HELD)));
}

#[test]
fn a_press_and_release_leaves_the_rest_level_and_the_learned_bottom_alone() {
    let mut run = Run::held_at_power_on();
    for _ in 0..3 {
        run.hold(CONTROL, 300, 30);
        run.hold(CONTROL, 600, 1);
        run.hold(CONTROL, 1000, 1);
        run.hold(CONTROL, 1150, 41);
        let bottom_level = run.bottom_level(CONTROL);
        run.hold(CONTROL, 600, 1);
        run.hold(CONTROL, 300, 4 * SETTLE_CYCLES);
        let control = run.scanner.sensor(CONTROL).unwrap();
        assert_eq!(control.rest_level(), 302);
        assert_eq!(control.single().rest_level, 302);
        assert_eq!(control.bottom_level(), bottom_level);
    }
    assert_ne!(run.bottom_level(CONTROL), 1200);
    assert_eq!(run.rest_level(HELD), HELD_REST);
}

#[test]
fn each_scan_mode_lowers_its_own_rest_level() {
    let mut run = Run::held_at_power_on();
    run.scanner.request_mode(Mode::Parallel);
    run.hold(HELD, HELD_LEVEL, SETTLING_CYCLES as usize + 1);
    assert_eq!(run.scanner.mode(), Some(Mode::Parallel));
    assert_eq!(run.rest_level(HELD), HELD_REST + 5);

    run.hold(HELD, 300, SETTLE_CYCLES + 4);
    let held = run.scanner.sensor(HELD).unwrap();
    let parallel_rest = held.rest_level();
    assert!((299..=303).contains(&parallel_rest), "{parallel_rest}");
    assert_eq!(held.parallel().rest_level, parallel_rest);
    assert_eq!(held.single().rest_level, HELD_REST);

    run.scanner.request_mode(Mode::Single);
    run.hold(HELD, 300, SETTLING_CYCLES as usize + 1);
    assert_eq!(run.scanner.mode(), Some(Mode::Single));
    assert_eq!(run.rest_level(HELD), HELD_REST);
    run.hold(HELD, 300, SETTLE_CYCLES + 4);
    let held = run.scanner.sensor(HELD).unwrap();
    let single_rest = held.single().rest_level;
    assert!((299..=303).contains(&single_rest), "{single_rest}");
    assert_eq!(held.rest_level(), held.single().rest_level);
    assert_eq!(held.parallel().rest_level, parallel_rest);
}

#[test]
fn a_lowered_rest_level_discards_what_the_bottom_out_learning_took_from_the_old_one() {
    let mut run = Run::held_at_power_on();
    run.deep_strokes(&[HELD, CONTROL]);
    assert_eq!(run.bottom_level(HELD), HELD_REST + 50);
    let control_bottom = run.bottom_level(CONTROL);
    assert!(control_bottom < HELD_REST + 50);

    run.hold(HELD, 300, SETTLE_CYCLES + 4);
    assert!((299..=303).contains(&run.rest_level(HELD)));
    assert_eq!(run.bottom_level(HELD), 1200);
    run.step(&[(HELD, 700), (CONTROL, 700)]);
    assert_eq!(run.bottom_level(HELD), 1200);
    assert_eq!(run.bottom_level(CONTROL), control_bottom);
}

#[test]
fn a_gap_in_the_readings_restarts_the_settle_time() {
    let mut run = Run::held_at_power_on();
    run.hold(HELD, 300, SETTLE_CYCLES - 4);
    run.scanner.set_raw_scanning(true);
    run.hold(HELD, 300, 10);
    run.scanner.set_raw_scanning(false);
    run.hold(HELD, 300, SETTLE_CYCLES - 1);
    assert_eq!(run.rest_level(HELD), HELD_REST);
    run.hold(HELD, 300, 2);
    assert!((299..=303).contains(&run.rest_level(HELD)));

    let mut run = Run::held_at_power_on();
    run.hold(HELD, 300, SETTLE_CYCLES - 4);
    run.scanner.request_mode(Mode::Parallel);
    run.scanner.request_mode(Mode::Single);
    run.hold(HELD, 300, SETTLING_CYCLES as usize + 1);
    assert!(!run.scanner.is_settling());
    assert_eq!(run.rest_level(HELD), HELD_REST);
}
