use openwoot_protocol::lightbar::{
    Blink, Comparison, Effect, Entry, MAX_GROUPS, MAX_HOLD_MS, MAX_TERMS, Meter, Operand, Signal,
    Term,
};

use super::entry::{Spot, set_term_time, term, term_mut, term_signal, term_time};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Field {
    Maximum,
    Period,
    MeterHost,
    Time(Spot),
    Constant(Spot),
    SignalHost(Spot),
    OperandHost(Spot),
    HostId,
    HostValue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Number {
    Milliseconds(u16),
    Value(f32),
    Id(u32),
}

impl Field {
    pub(super) fn all() -> Vec<Self> {
        let spots =
            (0..MAX_GROUPS).flat_map(|group| (0..MAX_TERMS).map(move |term| Spot { group, term }));
        let mut fields = vec![Self::Maximum, Self::Period, Self::MeterHost];
        for spot in spots {
            fields.extend([
                Self::Time(spot),
                Self::Constant(spot),
                Self::SignalHost(spot),
                Self::OperandHost(spot),
            ]);
        }
        fields.extend([Self::HostId, Self::HostValue]);
        fields
    }

    pub(super) const fn unit(self) -> &'static str {
        match self {
            Self::Time(_) | Self::Period => "s",
            Self::Maximum
            | Self::MeterHost
            | Self::Constant(_)
            | Self::SignalHost(_)
            | Self::OperandHost(_)
            | Self::HostId
            | Self::HostValue => "",
        }
    }

    pub(super) const fn placeholder(self) -> &'static str {
        match self {
            Self::HostId | Self::MeterHost | Self::SignalHost(_) | Self::OperandHost(_) => "Id",
            Self::HostValue => "Value",
            Self::Maximum | Self::Period | Self::Time(_) | Self::Constant(_) => "",
        }
    }

    pub(super) fn parse(self, text: &str) -> Result<Number, String> {
        let text = text.trim();
        match self {
            Self::Time(_) => seconds(text)
                .filter(|time| (1..=MAX_HOLD_MS).contains(time))
                .map(Number::Milliseconds)
                .ok_or_else(|| {
                    format!(
                        "`{text}` is not a time from 0.001 to {} s",
                        MAX_HOLD_MS / 1000
                    )
                }),
            Self::Period => seconds(text)
                .filter(|period| *period > 0)
                .map(Number::Milliseconds)
                .ok_or_else(|| format!("`{text}` is not a speed from 0.001 to 65 s")),
            Self::Maximum => text
                .parse::<f32>()
                .ok()
                .filter(|maximum| maximum.is_finite() && *maximum > 0.0)
                .map(Number::Value)
                .ok_or_else(|| format!("`{text}` is not a number above 0")),
            Self::Constant(_) | Self::HostValue => text
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
                .map(Number::Value)
                .ok_or_else(|| format!("`{text}` is not a number")),
            Self::MeterHost | Self::SignalHost(_) | Self::OperandHost(_) | Self::HostId => text
                .parse::<u32>()
                .map(Number::Id)
                .map_err(|_| format!("`{text}` is not a host signal id from 0 to {}", u32::MAX)),
        }
    }

    pub(super) fn text(self, entry: &Entry) -> Option<String> {
        match self {
            Self::Maximum => meter(entry).map(|meter| meter.maximum.to_string()),
            Self::Period => blink(entry).map(|blink| seconds_text(blink.period_ms)),
            Self::MeterHost => meter(entry).and_then(|meter| host_id(meter.signal)),
            Self::Time(spot) => term(entry, spot).and_then(term_time).map(seconds_text),
            Self::Constant(spot) => {
                comparison(entry, spot).and_then(|comparison| match comparison.operand {
                    Operand::Constant(constant) => Some(constant.to_string()),
                    Operand::Signal(_) => None,
                })
            }
            Self::SignalHost(spot) => term(entry, spot).and_then(|term| host_id(term_signal(term))),
            Self::OperandHost(spot) => {
                comparison(entry, spot).and_then(|comparison| match comparison.operand {
                    Operand::Signal(signal) => host_id(signal),
                    Operand::Constant(_) => None,
                })
            }
            Self::HostId | Self::HostValue => None,
        }
    }

    pub(super) fn apply(self, entry: &mut Entry, number: Number) {
        match (self, number) {
            (Self::Maximum, Number::Value(maximum)) => {
                if let Effect::Meter(meter) = &mut entry.effect {
                    meter.maximum = maximum;
                }
            }
            (Self::Period, Number::Milliseconds(period)) => {
                if let Effect::Blink(blink) = &mut entry.effect {
                    blink.period_ms = period;
                }
            }
            (Self::MeterHost, Number::Id(id)) => {
                if let Effect::Meter(meter) = &mut entry.effect
                    && matches!(meter.signal, Signal::Host(_))
                {
                    meter.signal = Signal::Host(id);
                }
            }
            (Self::Time(spot), Number::Milliseconds(time)) => {
                if let Some(term) = term_mut(entry, spot) {
                    set_term_time(term, time);
                }
            }
            (Self::Constant(spot), Number::Value(constant)) => {
                if let Some(Term::Compare(comparison)) = term_mut(entry, spot)
                    && matches!(comparison.operand, Operand::Constant(_))
                {
                    comparison.operand = Operand::Constant(constant);
                }
            }
            (Self::SignalHost(spot), Number::Id(id)) => {
                if let Some(term) = term_mut(entry, spot) {
                    match term {
                        Term::Trigger { signal, .. } | Term::Compare(Comparison { signal, .. }) => {
                            if matches!(signal, Signal::Host(_)) {
                                *signal = Signal::Host(id);
                            }
                        }
                    }
                }
            }
            (Self::OperandHost(spot), Number::Id(id)) => {
                if let Some(Term::Compare(comparison)) = term_mut(entry, spot)
                    && matches!(comparison.operand, Operand::Signal(Signal::Host(_)))
                {
                    comparison.operand = Operand::Signal(Signal::Host(id));
                }
            }
            _ => {}
        }
    }
}

pub(super) fn seconds_text(milliseconds: u16) -> String {
    let text = format!("{:.3}", f32::from(milliseconds) / 1000.0);
    let trimmed = text.trim_end_matches('0');
    if trimmed.ends_with('.') {
        format!("{trimmed}0")
    } else {
        trimmed.to_owned()
    }
}

pub(super) fn millis(value: f32) -> u16 {
    let clamped = value.round().clamp(0.0, f32::from(u16::MAX));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to the u16 range"
    )]
    let milliseconds = clamped as u16;
    milliseconds
}

fn seconds(text: &str) -> Option<u16> {
    let value = text.trim_end_matches('s').trim().parse::<f32>().ok()?;
    let milliseconds = (value * 1000.0).round();
    (milliseconds.is_finite() && milliseconds >= 0.5 && milliseconds <= f32::from(u16::MAX))
        .then(|| millis(milliseconds))
}

const fn meter(entry: &Entry) -> Option<&Meter> {
    match &entry.effect {
        Effect::Meter(meter) => Some(meter),
        _ => None,
    }
}

const fn blink(entry: &Entry) -> Option<&Blink> {
    match &entry.effect {
        Effect::Blink(blink) => Some(blink),
        _ => None,
    }
}

fn comparison(entry: &Entry, spot: Spot) -> Option<&Comparison> {
    match term(entry, spot)? {
        Term::Compare(comparison) => Some(comparison),
        Term::Trigger { .. } => None,
    }
}

fn host_id(signal: Signal) -> Option<String> {
    match signal {
        Signal::Host(id) => Some(id.to_string()),
        Signal::Builtin(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::lightbar::{Builtin, Comparator};

    use super::*;
    use crate::page::lightbar::entry::{Kind, Matrix, Timing, compare, single};

    const MATRIX: Matrix = Matrix {
        leds: 10,
        rows: 6,
        columns: 17,
    };
    const FIRST: Spot = Spot { group: 0, term: 0 };

    fn conditional(term: Term) -> Entry {
        Entry {
            condition: Some(single(term)),
            effect: Kind::Meter.effect(MATRIX),
        }
    }

    #[test]
    fn there_is_one_field_per_term_part() {
        let fields = Field::all();
        assert_eq!(fields.len(), 5 + 4 * MAX_GROUPS * MAX_TERMS);
        for (index, field) in fields.iter().enumerate() {
            assert!(!fields[index + 1..].contains(field), "{field:?}");
        }
    }

    #[test]
    fn times_and_periods_refuse_zero_and_out_of_range_values() {
        assert_eq!(
            Field::Time(FIRST).parse(" 1.5 "),
            Ok(Number::Milliseconds(1500))
        );
        assert_eq!(
            Field::Time(FIRST).parse("0.25 s"),
            Ok(Number::Milliseconds(250))
        );
        assert!(Field::Time(FIRST).parse("0").is_err());
        assert!(Field::Time(FIRST).parse("60.001").is_err());
        assert!(Field::Time(FIRST).parse("soon").is_err());
        assert_eq!(
            Field::Period.parse("65.535"),
            Ok(Number::Milliseconds(65535))
        );
        assert!(Field::Period.parse("0").is_err());
        assert!(Field::Period.parse("66").is_err());
        assert!(Field::Maximum.parse("0").is_err());
        assert!(Field::Maximum.parse("-3").is_err());
        assert!(Field::Maximum.parse("inf").is_err());
        assert_eq!(Field::Maximum.parse("2.5"), Ok(Number::Value(2.5)));
        assert_eq!(Field::Constant(FIRST).parse("-1"), Ok(Number::Value(-1.0)));
        assert!(Field::HostId.parse("-1").is_err());
        assert_eq!(Field::HostId.parse("42"), Ok(Number::Id(42)));
    }

    #[test]
    fn fields_read_and_write_the_entry() {
        let mut entry =
            conditional(Timing::Matched.apply(&compare(Signal::Host(7), Comparator::Less, 3.0)));
        assert_eq!(Field::Time(FIRST).text(&entry).as_deref(), Some("1.5"));
        assert_eq!(Field::Constant(FIRST).text(&entry).as_deref(), Some("3"));
        assert_eq!(Field::SignalHost(FIRST).text(&entry).as_deref(), Some("7"));
        assert_eq!(Field::OperandHost(FIRST).text(&entry), None);
        assert_eq!(Field::Maximum.text(&entry).as_deref(), Some("255"));
        Field::Time(FIRST).apply(&mut entry, Number::Milliseconds(200));
        Field::Constant(FIRST).apply(&mut entry, Number::Value(0.5));
        Field::SignalHost(FIRST).apply(&mut entry, Number::Id(9));
        Field::Maximum.apply(&mut entry, Number::Value(30.0));
        assert_eq!(Field::Time(FIRST).text(&entry).as_deref(), Some("0.2"));
        assert_eq!(Field::Constant(FIRST).text(&entry).as_deref(), Some("0.5"));
        assert_eq!(Field::SignalHost(FIRST).text(&entry).as_deref(), Some("9"));
        assert_eq!(Field::Maximum.text(&entry).as_deref(), Some("30"));
    }

    #[test]
    fn builtin_signals_keep_their_identity() {
        let mut entry = conditional(compare(
            Signal::Builtin(Builtin::CapsLock),
            Comparator::Equal,
            1.0,
        ));
        Field::SignalHost(FIRST).apply(&mut entry, Number::Id(3));
        Field::MeterHost.apply(&mut entry, Number::Id(3));
        Field::Time(FIRST).apply(&mut entry, Number::Milliseconds(5));
        assert_eq!(
            entry
                .condition
                .as_ref()
                .and_then(|condition| condition.groups[0].terms.first())
                .copied(),
            Some(compare(
                Signal::Builtin(Builtin::CapsLock),
                Comparator::Equal,
                1.0
            ))
        );
        assert_eq!(Field::MeterHost.text(&entry), None);
        assert_eq!(Field::Time(FIRST).text(&entry), None);
    }

    #[test]
    fn seconds_read_like_wootility() {
        assert_eq!(seconds_text(3000), "3.0");
        assert_eq!(seconds_text(1500), "1.5");
        assert_eq!(seconds_text(1250), "1.25");
        assert_eq!(seconds_text(1), "0.001");
        assert_eq!(millis(-3.0), 0);
        assert_eq!(millis(1e9), u16::MAX);
    }
}
