use openwoot::analog::{Switch, SwitchModel};

const fn kind(id: u8) -> SwitchModel {
    match SwitchModel::new(id) {
        Some(kind) => kind,
        None => SwitchModel::DEFAULT,
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
