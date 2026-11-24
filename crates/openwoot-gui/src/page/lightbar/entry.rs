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

pub(super) const DEFAULT_TIME_MS: u16 = 1500;
pub(super) const KINDS: [Kind; 6] = [
    Kind::Meter,
    Kind::ProfileIndicator,
    Kind::Mirror,
    Kind::Blink,
    Kind::Solid,
    Kind::Custom,
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
pub(super) const LOGICS: [Logic; 3] = [Logic::Single, Logic::All, Logic::Any];
pub(super) const TIMINGS: [Timing; 3] = [Timing::While, Timing::Matched, Timing::Changed];

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
            Self::Solid => "Solid",
            Self::Custom => "Multiple colours",
        }
    }

    pub(super) const fn description(self) -> &'static str {
        match self {
            Self::Meter => "Fills the LEDs like a bar with the value of a signal.",
            Self::ProfileIndicator => "Shows the colour and slot of the active profile.",
            Self::Mirror => "Copies the colours of a row or column of keys onto the LEDs.",
            Self::Blink => "Changes between two colours, sharply or smoothly.",
            Self::Solid => "Lights the LEDs, or a rectangle of keys, in one colour.",
            Self::Custom => "Gives every LED its own colour.",
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
            Self::While => "While it matches",
            Self::Matched => "When it starts to match",
            Self::Changed => "When it changes",
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
        Builtin::ActiveProfile => "Active Profile",
        Builtin::AnalogLevel => "Max Analog Value",
        Builtin::KeyPressed => "Any Key Actuated",
        Builtin::CapsLock => "Caps Lock State",
        Builtin::ScrollLock => "Scroll Lock State",
        Builtin::NumLock => "Num Lock State",
        Builtin::ProfileCount => "Profile Count",
        Builtin::ActiveLayer => "Active Fn Layer",
        Builtin::Brightness => "Brightness",
        Builtin::KeyRate => "Keystrokes Per Second",
        Builtin::ProfileColor => "Profile Colour",
    }
}

pub(super) const fn builtin_detail(builtin: Builtin) -> &'static str {
    match builtin {
        Builtin::ActiveProfile => "The number of the active profile, 1 for the first.",
        Builtin::AnalogLevel => "How deep the deepest key is pressed, 0 to 255.",
        Builtin::KeyPressed => "1 while any key is pressed, otherwise 0.",
        Builtin::CapsLock => "1 while Caps Lock is on, otherwise 0.",
        Builtin::ScrollLock => "1 while Scroll Lock is on, otherwise 0.",
        Builtin::NumLock => "1 while Num Lock is on, otherwise 0.",
        Builtin::ProfileCount => "The number of profiles on the keyboard.",
        Builtin::ActiveLayer => "The Fn layer in use, 0 for the base layer.",
        Builtin::Brightness => "The key brightness in percent, 0 to 100.",
        Builtin::KeyRate => "The key presses of the last second.",
        Builtin::ProfileColor => "The colour of the active profile, packed into one number.",
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

pub(super) const fn logic_name(logic: Logic) -> &'static str {
    match logic {
        Logic::Single => "Single condition",
        Logic::All => "All conditions required",
        Logic::Any => "Any condition required",
    }
}

pub(super) fn effect_summary(effect: &Effect) -> String {
    let leds = |range: &Range| {
        format!(
            "LEDs {}–{}",
            u16::from(range.first) + 1,
            u16::from(range.last) + 1
        )
    };
    let target = |target: &Target| match target {
        Target::Lightbar(range) => leds(range),
        Target::Keys(rect) => keys(*rect),
    };
    match effect {
        Effect::Meter(meter) => format!("{} · {}", signal_name(meter.signal), leds(&meter.range)),
        Effect::ProfileIndicator(indicator) => {
            format!(
                "{} · {}",
                background_name(indicator.background),
                leds(&indicator.range)
            )
        }
        Effect::Mirror(mirror) => format!("{} · {}", keys(mirror.source), leds(&mirror.range)),
        Effect::Blink(blink) => format!("{} ms · {}", blink.period_ms, target(&blink.target)),
        Effect::Solid(solid) => target(&solid.target),
        Effect::Custom(custom) => leds(&custom.range),
    }
}

pub(super) fn keys(rect: KeyRect) -> String {
    format!(
        "keys row {} column {} to row {} column {}",
        u16::from(rect.first.row) + 1,
        u16::from(rect.first.column) + 1,
        u16::from(rect.last.row) + 1,
        u16::from(rect.last.column) + 1
    )
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

pub(super) const fn background_name(background: Background) -> &'static str {
    match background {
        Background::ProfileColor => "Full",
        Background::Transparent => "Transparent",
        Background::Fixed(_) => "Fixed",
        Background::Contrast => "Complementing",
        Background::DimmedProfileColor => "Darker",
    }
}

fn term_summary(term: &Term) -> String {
    match term {
        Term::Trigger {
            signal,
            duration_ms,
        } => format!("{duration_ms} ms after {} changes", signal_name(*signal)),
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
                Some(hold) => format!("{hold} ms after {compared}"),
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
            for kind in KINDS {
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
        assert_eq!(condition_summary(&condition), "Caps Lock State = 1");
        add_group(&mut condition);
        condition.logic = Logic::Any;
        assert_eq!(
            condition_summary(&condition),
            "Caps Lock State = 1 or Any Key Actuated = 1"
        );
        assert_eq!(
            effect_summary(&Kind::Meter.effect(MATRIX)),
            "Max Analog Value · LEDs 1–10"
        );
    }
}
