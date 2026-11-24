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

    pub(super) const fn step(self) -> f64 {
        match self {
            Self::Time(_) | Self::Period => 100.0,
            Self::Maximum
            | Self::MeterHost
            | Self::Constant(_)
            | Self::SignalHost(_)
            | Self::OperandHost(_)
            | Self::HostId
            | Self::HostValue => 1.0,
        }
    }

    pub(super) fn parse(self, text: &str) -> Result<Number, String> {
        let text = text.trim();
        match self {
            Self::Time(_) => text
                .parse::<u16>()
                .ok()
                .filter(|time| (1..=MAX_HOLD_MS).contains(time))
                .map(Number::Milliseconds)
                .ok_or_else(|| format!("`{text}` is not a time from 1 to {MAX_HOLD_MS} ms")),
            Self::Period => text
                .parse::<u16>()
                .ok()
                .filter(|period| *period > 0)
                .map(Number::Milliseconds)
                .ok_or_else(|| format!("`{text}` is not a period from 1 to {} ms", u16::MAX)),
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
            Self::Period => blink(entry).map(|blink| blink.period_ms.to_string()),
            Self::MeterHost => meter(entry).and_then(|meter| host_id(meter.signal)),
            Self::Time(spot) => term(entry, spot)
                .and_then(term_time)
                .map(|time| time.to_string()),
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
            Field::Time(FIRST).parse(" 1500 "),
            Ok(Number::Milliseconds(1500))
        );
        assert!(Field::Time(FIRST).parse("0").is_err());
        assert!(Field::Time(FIRST).parse("60001").is_err());
        assert!(Field::Time(FIRST).parse("soon").is_err());
        assert_eq!(
            Field::Period.parse("65535"),
            Ok(Number::Milliseconds(65535))
        );
        assert!(Field::Period.parse("0").is_err());
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
        assert_eq!(Field::Time(FIRST).text(&entry).as_deref(), Some("1500"));
        assert_eq!(Field::Constant(FIRST).text(&entry).as_deref(), Some("3"));
        assert_eq!(Field::SignalHost(FIRST).text(&entry).as_deref(), Some("7"));
        assert_eq!(Field::OperandHost(FIRST).text(&entry), None);
        assert_eq!(Field::Maximum.text(&entry).as_deref(), Some("255"));
        Field::Time(FIRST).apply(&mut entry, Number::Milliseconds(200));
        Field::Constant(FIRST).apply(&mut entry, Number::Value(0.5));
        Field::SignalHost(FIRST).apply(&mut entry, Number::Id(9));
        Field::Maximum.apply(&mut entry, Number::Value(30.0));
        assert_eq!(Field::Time(FIRST).text(&entry).as_deref(), Some("200"));
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
}
