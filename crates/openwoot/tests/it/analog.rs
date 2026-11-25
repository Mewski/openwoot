use openwoot::analog::{CurveCache, CurveTable, Switch, SwitchModel};

const POINTS: usize = 4096;
const VALUE_MAX: u32 = 0x3FFF;
const TRAVEL_MAX: u32 = 4095;
const REST_MARGIN: u16 = 5;
const STEPS: &[u8] = include_bytes!("../../data/travel_curves.txt");
const CURVE_OF_MODEL: [usize; SwitchModel::COUNT as usize] = [
    0, 1, 2, 3, 4, 5, 5, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
    24, 16, 16, 6, 20, 2, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 35, 36, 37, 38, 39,
];
const LEVELS: [(u16, u16); 4] = [(300, 1200), (0, 4095), (2000, 2050), (1100, 1008)];

const fn kind(id: u8) -> SwitchModel {
    match SwitchModel::new(id) {
        Some(kind) => kind,
        None => SwitchModel::DEFAULT,
    }
}

fn full_curves() -> Vec<[u16; POINTS]> {
    let (lines, _) = STEPS.as_chunks::<POINTS>();
    lines
        .iter()
        .map(|line| {
            let mut points = [0; POINTS];
            let mut value = 0;
            for (point, &step) in points.iter_mut().skip(1).zip(line) {
                value += u16::from(step - b'A');
                *point = value;
            }
            points
        })
        .collect()
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the reference truncates to 16 bits as the full-table firmware did"
)]
fn full_interpolate(points: &[u16; POINTS], index_q8: u32) -> u16 {
    let point = |index: u32| {
        usize::try_from(index)
            .ok()
            .and_then(|index| points.get(index))
            .copied()
            .unwrap_or(0)
    };
    let index = (index_q8 >> 8) & 0xFFFF;
    let fraction = index_q8 & 0xFF;
    let low = u32::from(point(index));
    if fraction == 0 {
        return low as u16;
    }
    let high = u32::from(point(index + 1));
    (low + (high
        .wrapping_sub(low)
        .wrapping_mul(fraction)
        .wrapping_add(128)
        >> 8)) as u16
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the reference truncates to 16 bits as the full-table firmware did"
)]
fn full_travel(
    points: &[u16; POINTS],
    switch: &Switch,
    value: u16,
    rest_level: u16,
    bottom_level: u16,
) -> u16 {
    if value <= rest_level.wrapping_add(REST_MARGIN) {
        return 0;
    }
    let rest = u32::from(rest_level);
    let range = u32::from(bottom_level.saturating_sub(rest_level));
    let curve_end = switch.curve_end();
    let index_q8 = ((range >> 1)
        .wrapping_add(curve_end.wrapping_mul(u32::from(value).wrapping_sub(rest))))
    .checked_div(range)
    .unwrap_or(0)
    .min(curve_end);
    let mut travel = u32::from(full_interpolate(points, index_q8));
    if switch.travel_offset().is_some() {
        let end_value = u32::from(full_interpolate(points, curve_end));
        travel = (VALUE_MAX * travel)
            .checked_div(end_value)
            .unwrap_or(0)
            .min(VALUE_MAX);
    }
    (TRAVEL_MAX * travel / VALUE_MAX) as u16
}

fn assert_travels_follow_the_full_curve(kind: SwitchModel, points: &[u16; POINTS]) {
    let mut tables = [CurveTable::EMPTY];
    let mut cache = CurveCache::new(&mut tables);
    let travel = kind.travel();
    for offset in [None, Some(0), Some(travel / 3), Some(travel - 1)] {
        let switch = Switch::new(kind, offset);
        cache.assign([&switch]);
        for (rest_level, bottom_level) in LEVELS {
            for value in 0..=u16::MAX {
                let expected = full_travel(points, &switch, value, rest_level, bottom_level);
                let context = (kind.id(), offset, value, rest_level, bottom_level);
                assert_eq!(
                    switch.travel(value, rest_level, bottom_level),
                    expected,
                    "{context:?}"
                );
                assert_eq!(
                    cache.travel(&switch, value, rest_level, bottom_level),
                    Some(expected),
                    "{context:?}"
                );
            }
        }
    }
}

#[test]
fn switch_models_exist_for_49_ids() {
    assert!(SwitchModel::new(48).is_some());
    assert!(SwitchModel::new(49).is_none());
    assert_eq!(kind(34).travel(), 80);
    assert_eq!(kind(24).nominal_bottom(), 1450);
    assert_eq!(kind(14).learn_strength(), 114);
    assert_eq!(kind(14).group(), 14);
}

#[test]
fn a_switch_without_travel_offset_uses_the_whole_curve() {
    let switch = Switch::new(SwitchModel::DEFAULT, None);
    assert_eq!(switch.curve_end(), 0xF_FF00);
    assert_eq!(switch.learn_strength(), 255);
    assert_eq!(switch.nominal_bottom(), 1200);
    assert_eq!(switch.bottom_level(300), 1200);
    assert_eq!(switch.travel(100, 300, 1200), 0);
    assert_eq!(switch.travel(305, 300, 1200), 0);
    assert_eq!(switch.travel(700, 300, 1200), 3023);
    assert_eq!(switch.travel(1500, 300, 1200), 4095);
}

#[test]
fn a_travel_offset_shortens_the_curve_and_weakens_the_learning() {
    let switch = Switch::new(kind(2), Some(10));
    assert_eq!(switch.curve_end(), 706_560);
    assert_eq!(switch.learn_strength(), 37);
    assert_eq!(switch.nominal_bottom(), 900);
    assert_eq!(switch.bottom_level(300), 704);
    assert_eq!(switch.travel(306, 300, 1000), 84);
    assert_eq!(switch.travel(650, 300, 1000), 2864);
    assert_eq!(switch.travel(1400, 300, 1000), 4095);
}

#[test]
fn the_bottom_level_keeps_fifty_counts_above_the_rest_level() {
    let switch = Switch::new(SwitchModel::DEFAULT, None);
    assert_eq!(switch.bottom_level(1180), 1230);
    assert_eq!(switch.bottom_level(1300), 1350);
}

#[test]
fn a_zero_range_between_rest_and_bottom_reads_the_start_of_the_curve() {
    let switch = Switch::new(SwitchModel::DEFAULT, None);
    assert_eq!(switch.travel(900, 300, 300), 0);
}

#[test]
fn travel_per_tenth_millimetre_follows_the_remaining_travel() {
    assert_eq!(
        Switch::new(SwitchModel::DEFAULT, None).travel_per_tenth_millimetre(),
        102
    );
    assert_eq!(
        Switch::new(kind(2), Some(10)).travel_per_tenth_millimetre(),
        146
    );
    assert_eq!(
        Switch::new(kind(2), Some(66)).travel_per_tenth_millimetre(),
        0
    );
}

#[test]
fn a_bottom_level_below_the_rest_level_reads_no_travel() {
    let switch = Switch::new(SwitchModel::DEFAULT, None);
    assert_eq!(switch.travel(1150, 1100, 1008), 0);
}

#[test]
fn a_travel_offset_at_or_beyond_the_switch_travel_leaves_no_travel() {
    for offset in [66, 67, 200] {
        let switch = Switch::new(kind(2), Some(offset));
        assert_eq!(switch.curve_end(), 0);
        assert_eq!(switch.learn_strength(), 0);
        assert_eq!(switch.travel(900, 300, 1000), 0);
        assert_eq!(switch.travel_per_tenth_millimetre(), 0);
    }
}

#[test]
fn travel_through_a_curve_table_and_the_compact_curve_equals_the_full_curve() {
    let curves = full_curves();
    std::thread::scope(|scope| {
        for (kind, curve) in (0..SwitchModel::COUNT)
            .filter_map(SwitchModel::new)
            .zip(CURVE_OF_MODEL)
        {
            let points = &curves[curve];
            scope.spawn(move || assert_travels_follow_the_full_curve(kind, points));
        }
    });
}

#[test]
fn a_cache_keeps_the_most_used_curves_and_hands_a_released_table_to_the_next_curve() {
    let mut tables = [CurveTable::EMPTY];
    let mut cache = CurveCache::new(&mut tables);
    let common = Switch::new(kind(0), None);
    let rare = Switch::new(kind(2), Some(10));
    cache.assign([&rare, &common, &common]);
    assert_eq!(cache.travel(&rare, 650, 300, 1000), None);
    assert_eq!(
        cache.travel(&common, 700, 300, 1200),
        Some(common.travel(700, 300, 1200))
    );

    cache.assign([&rare]);
    assert_eq!(cache.travel(&common, 700, 300, 1200), None);
    assert_eq!(cache.travel(&rare, 650, 300, 1000), Some(2864));
    assert_eq!(rare.travel(650, 300, 1000), 2864);
}
