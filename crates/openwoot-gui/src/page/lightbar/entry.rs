use gpui_kit::assets::IconName as Catalog;
use openwoot_protocol::{
    heapless,
    keys::Position,
    lightbar::{
        Background, Blink, Builtin, Comparator, Comparison, Condition, Custom, Effect, Entry,
        Group, KeyRect, Logic, MAX_GROUPS, MAX_LEDS, MAX_TERMS, Meter, Mirror, Operand,
        ProfileIndicator, Range, Signal, Solid, Target, Term, Waveform,
    },
    lighting::Color,
};

use super::number::seconds_text;

pub(super) const DEFAULT_TIME_MS: u16 = 1500;
pub(super) const TYPES: [Kind; 5] = [
    Kind::ProfileIndicator,
    Kind::Meter,
    Kind::Mirror,
    Kind::Blink,
    Kind::Solid,
];
pub(super) const BUILTINS: [Builtin; 11] = [
    Builtin::ActiveProfile,
    Builtin::AnalogLevel,
    Builtin::KeyPressed,
    Builtin::CapsLock,
    Builtin::ScrollLock,
    Builtin::NumLock,
    Builtin::ProfileCount,
    Builtin::ActiveLayer,
    Builtin::Brightness,
    Builtin::KeyRate,
    Builtin::ProfileColor,
];
pub(super) const COMPARATORS: [Comparator; 6] = [
    Comparator::Equal,
    Comparator::NotEqual,
    Comparator::Greater,
    Comparator::GreaterOrEqual,
    Comparator::Less,
    Comparator::LessOrEqual,
];
pub(super) const TIMINGS: [Timing; 3] = [Timing::Matched, Timing::While, Timing::Changed];

const RED: Color = color(255, 0, 0);
const WHITE: Color = color(255, 255, 255);
const BLACK: Color = color(0, 0, 0);
const NUMBER_ROW: u8 = 1;
const NUMBER_ROW_LAST_COLUMN: u8 = 10;
const PULSE_PERIOD_MS: u16 = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Meter,
    ProfileIndicator,
    Mirror,
    Blink,
    Solid,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Timing {
    While,
    Matched,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Matrix {
    pub(super) leds: u8,
    pub(super) rows: u8,
    pub(super) columns: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Spot {
    pub(super) group: usize,
    pub(super) term: usize,
}

impl Kind {
    pub(super) const fn of(effect: &Effect) -> Self {
        match effect {
            Effect::Meter(_) => Self::Meter,
            Effect::ProfileIndicator(_) => Self::ProfileIndicator,
            Effect::Mirror(_) => Self::Mirror,
            Effect::Blink(_) => Self::Blink,
            Effect::Solid(_) => Self::Solid,
            Effect::Custom(_) => Self::Custom,
        }
    }

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Meter => "Progress bar",
            Self::ProfileIndicator => "Profile indicator",
            Self::Mirror => "Sync with keyboard",
            Self::Blink => "Pulse",
            Self::Solid | Self::Custom => "Solid",
        }
    }

    pub(super) const fn icon(self) -> Catalog {
        match self {
            Self::Meter => Catalog::Gauge,
            Self::ProfileIndicator => Catalog::ToggleRight,
            Self::Mirror => Catalog::RefreshCw,
            Self::Blink => Catalog::HeartPulse,
            Self::Solid | Self::Custom => Catalog::PaintBucket,
        }
    }

    pub(super) const fn family(self) -> Self {
        match self {
            Self::Custom => Self::Solid,
            kind => kind,
        }
    }

    pub(super) fn effect(self, matrix: Matrix) -> Effect {
        let range = full_range(matrix.leds);
        match self {
            Self::Meter => Effect::Meter(meter(Builtin::AnalogLevel, RED, None, range)),
            Self::ProfileIndicator => Effect::ProfileIndicator(ProfileIndicator {
                range,
                background: Background::DimmedProfileColor,
                segments: 1,
            }),
            Self::Mirror => Effect::Mirror(Mirror {
                range,
                source: number_row(matrix),
            }),
            Self::Blink => Effect::Blink(Blink {
                from: BLACK,
                to: WHITE,
                period_ms: PULSE_PERIOD_MS,
                waveform: Waveform::Triangle,
                target: Target::Lightbar(range),
            }),
            Self::Solid => Effect::Solid(Solid {
                color: WHITE,
                target: Target::Lightbar(range),
            }),
            Self::Custom => Effect::Custom(Custom {
                colors: (0..usize::from(matrix.leds).min(MAX_LEDS))
                    .map(|_| WHITE.rgb())
                    .collect(),
                range,
            }),
        }
    }
}

impl Timing {
    pub(super) const fn of(term: &Term) -> Self {
        match term {
            Term::Trigger { .. } => Self::Changed,
            Term::Compare(Comparison { hold_ms: None, .. }) => Self::While,
            Term::Compare(Comparison {
                hold_ms: Some(_), ..
            }) => Self::Matched,
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::While => "While match",
            Self::Matched => "On match",
            Self::Changed => "On change",
        }
    }

    pub(super) fn apply(self, term: &Term) -> Term {
        let signal = term_signal(term);
        let time = term_time(term).unwrap_or(DEFAULT_TIME_MS);
        let comparison = match term {
            Term::Compare(comparison) => *comparison,
            Term::Trigger { .. } => Comparison {
                signal,
                comparator: Comparator::Equal,
                operand: Operand::Constant(1.0),
                hold_ms: None,
            },
        };
        match self {
            Self::While => Term::Compare(Comparison {
                hold_ms: None,
                ..comparison
            }),
            Self::Matched => Term::Compare(Comparison {
                hold_ms: Some(u32::from(time)),
                ..comparison
            }),
            Self::Changed => Term::Trigger {
                signal,
                duration_ms: time,
            },
        }
    }
}

pub(super) const fn full_range(leds: u8) -> Range {
    Range {
        first: 0,
        last: leds.saturating_sub(1),
    }
}

pub(super) const fn color(red: u8, green: u8, blue: u8) -> Color {
    Color {
        red,
        green,
        blue,
        alpha: Color::OPAQUE,
    }
}

pub(super) const fn meter(
    signal: Builtin,
    color: Color,
    gradient: Option<Color>,
    range: Range,
) -> Meter {
    Meter {
        signal: Signal::Builtin(signal),
        maximum: maximum_of(Signal::Builtin(signal)),
        color,
        background: None,
        gradient_start: gradient,
        range,
        reverse: false,
    }
}

pub(super) const fn compare(signal: Signal, comparator: Comparator, constant: f32) -> Term {
    Term::Compare(Comparison {
        signal,
        comparator,
        operand: Operand::Constant(constant),
        hold_ms: None,
    })
}

pub(super) fn single(term: Term) -> Condition {
    let mut terms = heapless::Vec::new();
    terms.push(term).ok();
    let mut groups = heapless::Vec::new();
    groups
        .push(Group {
            logic: Logic::Single,
            terms,
        })
        .ok();
    Condition {
        logic: Logic::Single,
        groups,
    }
}

pub(super) fn default_condition() -> Condition {
    single(compare(
        Signal::Builtin(Builtin::CapsLock),
        Comparator::Equal,
        1.0,
    ))
}

pub(super) fn number_row(matrix: Matrix) -> KeyRect {
    let row = NUMBER_ROW.min(matrix.rows.saturating_sub(1));
    let last = NUMBER_ROW_LAST_COLUMN.min(matrix.columns.saturating_sub(1));
    KeyRect {
        first: Position {
            row,
            column: 1.min(last),
        },
        last: Position { row, column: last },
    }
}

pub(super) fn above_keys(matrix: Matrix) -> KeyRect {
    KeyRect {
        first: Position { row: 0, column: 0 },
        last: Position {
            row: 0,
            column: matrix.columns.saturating_sub(1),
        },
    }
}

pub(super) const fn period_ms(effect: &Effect) -> Option<u16> {
    match effect {
        Effect::Blink(blink) => Some(blink.period_ms),
        _ => None,
    }
}

pub(super) const fn set_period_ms(effect: &mut Effect, period: u16) {
    if let Effect::Blink(blink) = effect {
        blink.period_ms = period;
    }
}

pub(super) fn add_group(condition: &mut Condition) {
    let mut terms = heapless::Vec::new();
    terms.push(default_term()).ok();
    let added = condition
        .groups
        .push(Group {
            logic: Logic::Single,
            terms,
        })
        .is_ok();
    if added && condition.logic == Logic::Single && condition.groups.len() > 1 {
        condition.logic = Logic::All;
    }
}

pub(super) fn add_term(group: &mut Group) {
    let added = group.terms.push(default_term()).is_ok();
    if added && group.logic == Logic::Single && group.terms.len() > 1 {
        group.logic = Logic::All;
    }
}

pub(super) fn remove_term(condition: &mut Condition, group: usize, term: usize) -> bool {
    let Some(members) = condition.groups.get_mut(group) else {
        return true;
    };
    if term < members.terms.len() {
        members.terms.remove(term);
    }
    if members.terms.is_empty() {
        remove_group(condition, group);
    }
    !condition.groups.is_empty()
}

pub(super) fn remove_group(condition: &mut Condition, group: usize) -> bool {
    if group < condition.groups.len() {
        condition.groups.remove(group);
    }
    !condition.groups.is_empty()
}

pub(super) fn can_add_group(condition: &Condition) -> bool {
    condition.groups.len() < MAX_GROUPS
}

pub(super) fn can_add_term(group: &Group) -> bool {
    group.terms.len() < MAX_TERMS
}

pub(super) const fn term_signal(term: &Term) -> Signal {
    match term {
        Term::Trigger { signal, .. } | Term::Compare(Comparison { signal, .. }) => *signal,
    }
}

pub(super) const fn set_term_signal(term: &mut Term, signal: Signal) {
    match term {
        Term::Trigger {
            signal: current, ..
        }
        | Term::Compare(Comparison {
            signal: current, ..
        }) => *current = signal,
    }
}

pub(super) fn term_time(term: &Term) -> Option<u16> {
    match term {
        Term::Trigger { duration_ms, .. } => Some(*duration_ms),
        Term::Compare(Comparison { hold_ms, .. }) => {
            hold_ms.map(|hold| u16::try_from(hold).unwrap_or(u16::MAX))
        }
    }
}

pub(super) fn set_term_time(term: &mut Term, time: u16) {
    match term {
        Term::Trigger { duration_ms, .. } => *duration_ms = time,
        Term::Compare(Comparison {
            hold_ms: Some(hold),
            ..
        }) => *hold = u32::from(time),
        Term::Compare(Comparison { hold_ms: None, .. }) => {}
    }
}

pub(super) const fn maximum_of(signal: Signal) -> f32 {
    match signal {
        Signal::Builtin(Builtin::AnalogLevel) => 255.0,
        Signal::Builtin(Builtin::Brightness) | Signal::Host(_) => 100.0,
        Signal::Builtin(Builtin::KeyRate) => 30.0,
        Signal::Builtin(Builtin::ActiveProfile | Builtin::ProfileCount) => 4.0,
        Signal::Builtin(Builtin::ActiveLayer) => 3.0,
        Signal::Builtin(
            Builtin::KeyPressed
            | Builtin::CapsLock
            | Builtin::ScrollLock
            | Builtin::NumLock
            | Builtin::ProfileColor,
        ) => 1.0,
    }
}

pub(super) fn signal_name(signal: Signal) -> String {
    match signal {
        Signal::Builtin(builtin) => builtin_name(builtin).to_owned(),
        Signal::Host(id) => format!("Host signal {id}"),
    }
}

pub(super) const fn builtin_name(builtin: Builtin) -> &'static str {
    match builtin {
        Builtin::ActiveProfile => "Active profile",
        Builtin::AnalogLevel => "Max analog value",
        Builtin::KeyPressed => "Any key actuated",
        Builtin::CapsLock => "Caps Lock state",
        Builtin::ScrollLock => "Scroll Lock state",
        Builtin::NumLock => "Num Lock state",
        Builtin::ProfileCount => "Profile count",
        Builtin::ActiveLayer => "Active Fn layer",
        Builtin::Brightness => "Brightness",
        Builtin::KeyRate => "Keystrokes per second",
        Builtin::ProfileColor => "Profile colour",
    }
}

pub(super) const fn comparator_symbol(comparator: Comparator) -> &'static str {
    match comparator {
        Comparator::Equal => "=",
        Comparator::NotEqual => "≠",
        Comparator::Greater => ">",
        Comparator::GreaterOrEqual => "≥",
        Comparator::Less => "<",
        Comparator::LessOrEqual => "≤",
    }
}

pub(super) const fn lightbar_range(effect: &Effect) -> Option<Range> {
    match effect {
        Effect::Meter(Meter { range, .. })
        | Effect::ProfileIndicator(ProfileIndicator { range, .. })
        | Effect::Mirror(Mirror { range, .. })
        | Effect::Custom(Custom { range, .. })
        | Effect::Blink(Blink {
            target: Target::Lightbar(range),
            ..
        })
        | Effect::Solid(Solid {
            target: Target::Lightbar(range),
            ..
        }) => Some(*range),
        Effect::Blink(_) | Effect::Solid(_) => None,
    }
}

pub(super) fn term(entry: &Entry, spot: Spot) -> Option<&Term> {
    entry
        .condition
        .as_ref()?
        .groups
        .get(spot.group)?
        .terms
        .get(spot.term)
}

pub(super) fn term_mut(entry: &mut Entry, spot: Spot) -> Option<&mut Term> {
    entry
        .condition
        .as_mut()?
        .groups
        .get_mut(spot.group)?
        .terms
        .get_mut(spot.term)
}

pub(super) fn condition_summary(condition: &Condition) -> String {
    let joiner = |logic: Logic| match logic {
        Logic::Any => " or ",
        Logic::Single | Logic::All => " and ",
    };
    let groups: Vec<String> = condition
        .groups
        .iter()
        .map(|group| {
            let terms: Vec<String> = group.terms.iter().map(term_summary).collect();
            let text = terms.join(joiner(group.logic));
            if terms.len() > 1 && condition.groups.len() > 1 {
                format!("({text})")
            } else {
                text
            }
        })
        .collect();
    groups.join(joiner(condition.logic))
}

fn term_summary(term: &Term) -> String {
    match term {
        Term::Trigger {
            signal,
            duration_ms,
        } => format!(
            "{} changes, for {} s",
            signal_name(*signal),
            seconds_text(*duration_ms)
        ),
        Term::Compare(comparison) => {
            let operand = match comparison.operand {
                Operand::Signal(signal) => signal_name(signal),
                Operand::Constant(constant) => constant.to_string(),
            };
            let compared = format!(
                "{} {} {operand}",
                signal_name(comparison.signal),
                comparator_symbol(comparison.comparator)
            );
            match comparison.hold_ms {
                Some(hold) => format!(
                    "{compared}, for {} s",
                    seconds_text(u16::try_from(hold).unwrap_or(u16::MAX))
                ),
                None => compared,
            }
        }
    }
}

const fn default_term() -> Term {
    compare(Signal::Builtin(Builtin::KeyPressed), Comparator::Equal, 1.0)
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::configuration::{Capabilities, Invalid, Validate as _};

    use super::*;

    const MATRIX: Matrix = Matrix {
        leds: 10,
        rows: 6,
        columns: 17,
    };

    fn capabilities(leds: u8) -> Capabilities {
        Capabilities {
            key_rows: 6,
            key_columns: 17,
            profiles: 4,
            layouts: heapless::Vec::new(),
            lightbar_leds: leds,
            host_signals: false,
            switch_models: 1,
            edition: None,
        }
    }

    #[test]
    fn every_kind_starts_as_a_valid_effect() {
        for leds in [1, 2, 10, 16] {
            let matrix = Matrix { leds, ..MATRIX };
            for kind in TYPES.into_iter().chain([Kind::Custom]) {
                let effect = kind.effect(matrix);
                assert_eq!(Kind::of(&effect), kind);
                assert_eq!(
                    effect.validate(&capabilities(leds)),
                    Ok(()),
                    "{kind:?} {leds}"
                );
            }
        }
    }

    #[test]
    fn the_number_row_fits_small_matrices() {
        let rect = number_row(Matrix {
            leds: 4,
            rows: 1,
            columns: 1,
        });
        assert_eq!(rect.first, Position { row: 0, column: 0 });
        assert_eq!(rect.last, Position { row: 0, column: 0 });
        let rect = number_row(MATRIX);
        assert_eq!(rect.first, Position { row: 1, column: 1 });
        assert_eq!(rect.last, Position { row: 1, column: 10 });
    }

    #[test]
    fn a_second_member_turns_a_single_condition_into_all() {
        let mut condition = default_condition();
        assert_eq!(condition.validate(&capabilities(10)), Ok(()));
        add_group(&mut condition);
        assert_eq!(condition.logic, Logic::All);
        assert_eq!(condition.groups.len(), 2);
        add_group(&mut condition);
        assert_eq!(condition.groups.len(), MAX_GROUPS);
        let group = condition.groups.get_mut(0).unwrap();
        add_term(group);
        assert_eq!(group.logic, Logic::All);
        add_term(group);
        assert_eq!(group.terms.len(), MAX_TERMS);
        assert_eq!(condition.validate(&capabilities(10)), Ok(()));
    }

    #[test]
    fn removing_the_last_term_removes_its_group_and_then_the_condition() {
        let mut condition = default_condition();
        add_group(&mut condition);
        assert!(remove_term(&mut condition, 0, 0));
        assert_eq!(condition.groups.len(), 1);
        assert!(!remove_term(&mut condition, 0, 0));
        assert!(condition.groups.is_empty());
    }

    #[test]
    fn a_single_condition_with_two_groups_can_never_hold() {
        let mut condition = default_condition();
        add_group(&mut condition);
        condition.logic = Logic::Single;
        assert_eq!(
            condition.validate(&capabilities(10)),
            Err(Invalid::ConditionNeverHolds)
        );
    }

    #[test]
    fn timings_convert_between_the_three_kinds_of_terms() {
        let term = compare(
            Signal::Builtin(Builtin::Brightness),
            Comparator::Greater,
            5.0,
        );
        assert_eq!(Timing::of(&term), Timing::While);
        let matched = Timing::Matched.apply(&term);
        assert_eq!(Timing::of(&matched), Timing::Matched);
        assert_eq!(term_time(&matched), Some(DEFAULT_TIME_MS));
        let changed = Timing::Changed.apply(&matched);
        assert_eq!(
            changed,
            Term::Trigger {
                signal: Signal::Builtin(Builtin::Brightness),
                duration_ms: DEFAULT_TIME_MS
            }
        );
        let back = Timing::While.apply(&changed);
        assert_eq!(Timing::of(&back), Timing::While);
        assert_eq!(term_signal(&back), Signal::Builtin(Builtin::Brightness));
    }

    #[test]
    fn zero_and_long_times_are_refused() {
        let capabilities = capabilities(10);
        let mut trigger = Timing::Changed.apply(&default_term());
        set_term_time(&mut trigger, 0);
        let entry = Entry {
            condition: Some(single(trigger)),
            effect: Kind::Solid.effect(MATRIX),
        };
        assert_eq!(entry.validate(&capabilities), Err(Invalid::ZeroDuration));
        let mut held = Timing::Matched.apply(&default_term());
        set_term_time(&mut held, 60_001);
        let entry = Entry {
            condition: Some(single(held)),
            effect: Kind::Solid.effect(MATRIX),
        };
        assert_eq!(
            entry.validate(&capabilities),
            Err(Invalid::HoldTimeTooLong(60_001))
        );
    }

    #[test]
    fn summaries_read_like_the_condition() {
        let mut condition = single(compare(
            Signal::Builtin(Builtin::CapsLock),
            Comparator::Equal,
            1.0,
        ));
        assert_eq!(condition_summary(&condition), "Caps Lock state = 1");
        add_group(&mut condition);
        condition.logic = Logic::Any;
        assert_eq!(
            condition_summary(&condition),
            "Caps Lock state = 1 or Any key actuated = 1"
        );
        let mut changed = condition.clone();
        changed.groups.truncate(1);
        if let Some(group) = changed.groups.get_mut(0) {
            group.terms.clear();
            group
                .terms
                .push(Timing::Changed.apply(&default_term()))
                .ok();
        }
        assert_eq!(
            condition_summary(&changed),
            "Any key actuated changes, for 1.5 s"
        );
    }
}
