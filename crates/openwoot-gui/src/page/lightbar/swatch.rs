use openwoot_protocol::{
    lightbar::{Background, Effect, MAX_LEDS},
    lighting::{Color, Rgb},
};

const BLACK: Rgb = Rgb {
    red: 0,
    green: 0,
    blue: 0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Swatch {
    Main,
    Second,
    Third,
    Led,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shade {
    Color(Rgb),
    Off,
}

impl Shade {
    pub(super) const fn rgb(self) -> Option<Rgb> {
        match self {
            Self::Color(color) => Some(color),
            Self::Off => None,
        }
    }

    fn of(color: Option<Rgb>) -> Self {
        color.map_or(Self::Off, Self::Color)
    }
}

impl Swatch {
    pub(super) const ALL: [Self; 4] = [Self::Main, Self::Second, Self::Third, Self::Led];

    pub(super) const fn allows_none(self) -> bool {
        matches!(self, Self::Second | Self::Third)
    }

    pub(super) fn color(self, effect: &Effect, led: u8) -> Option<Shade> {
        let shown = match (self, effect) {
            (Self::Main, Effect::Meter(meter)) => Some(meter.color.rgb()),
            (Self::Second, Effect::Meter(meter)) => meter.background.map(Color::rgb),
            (Self::Third, Effect::Meter(meter)) => meter.gradient_start.map(Color::rgb),
            (Self::Main, Effect::ProfileIndicator(indicator)) => match indicator.background {
                Background::Fixed(color) => Some(color.rgb()),
                _ => return None,
            },
            (Self::Main, Effect::Blink(blink)) => Some(blink.to.rgb()),
            (Self::Second, Effect::Blink(blink)) => {
                Some(blink.from.rgb()).filter(|from| *from != BLACK)
            }
            (Self::Main, Effect::Solid(solid)) => Some(solid.color.rgb()),
            (Self::Led, Effect::Custom(custom)) => Some(
                custom
                    .colors
                    .get(usize::from(led))
                    .copied()
                    .unwrap_or(BLACK),
            ),
            _ => return None,
        };
        Some(Shade::of(shown))
    }

    pub(super) fn set(self, effect: &mut Effect, led: u8, leds: u8, color: Option<Rgb>) {
        let opaque = color.map(Rgb::opaque);
        match (self, effect, opaque) {
            (Self::Main, Effect::Meter(meter), Some(color)) => meter.color = color,
            (Self::Second, Effect::Meter(meter), color) => meter.background = color,
            (Self::Third, Effect::Meter(meter), color) => meter.gradient_start = color,
            (Self::Main, Effect::ProfileIndicator(indicator), Some(color)) => {
                indicator.background = Background::Fixed(color);
            }
            (Self::Main, Effect::Blink(blink), Some(color)) => blink.to = color,
            (Self::Second, Effect::Blink(blink), color) => {
                blink.from = color.unwrap_or(BLACK.opaque());
            }
            (Self::Main, Effect::Solid(solid), Some(color)) => solid.color = color,
            (Self::Led, Effect::Custom(custom), Some(color)) => {
                let wanted = usize::from(led) + 1;
                if wanted > usize::from(leds).min(MAX_LEDS) {
                    return;
                }
                while custom.colors.len() < wanted {
                    if custom.colors.push(BLACK).is_err() {
                        return;
                    }
                }
                if let Some(slot) = custom.colors.get_mut(usize::from(led)) {
                    *slot = color.rgb();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        configuration::{Capabilities, Validate as _},
        heapless,
    };

    use super::*;
    use crate::page::lightbar::entry::{Kind, Matrix, TYPES};

    const MATRIX: Matrix = Matrix {
        leds: 10,
        rows: 6,
        columns: 17,
    };
    const RED: Rgb = Rgb {
        red: 255,
        green: 0,
        blue: 0,
    };

    #[test]
    fn a_swatch_writes_what_it_reads() {
        for kind in TYPES.into_iter().chain([Kind::Custom]) {
            let mut effect = kind.effect(MATRIX);
            for swatch in Swatch::ALL {
                let before = swatch.color(&effect, 3);
                swatch.set(&mut effect, 3, MATRIX.leds, Some(RED));
                if before.is_some() {
                    assert_eq!(
                        swatch.color(&effect, 3),
                        Some(Shade::Color(RED)),
                        "{kind:?} {swatch:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn no_colour_clears_the_optional_colours_and_darkens_the_pulse() {
        let mut meter = Kind::Meter.effect(MATRIX);
        Swatch::Second.set(&mut meter, 0, MATRIX.leds, Some(RED));
        assert_eq!(Swatch::Second.color(&meter, 0), Some(Shade::Color(RED)));
        Swatch::Second.set(&mut meter, 0, MATRIX.leds, None);
        assert_eq!(Swatch::Second.color(&meter, 0), Some(Shade::Off));
        Swatch::Main.set(&mut meter, 0, MATRIX.leds, None);
        assert_ne!(Swatch::Main.color(&meter, 0), Some(Shade::Off));
        let mut pulse = Kind::Blink.effect(MATRIX);
        assert_eq!(Swatch::Second.color(&pulse, 0), Some(Shade::Off));
        Swatch::Second.set(&mut pulse, 0, MATRIX.leds, Some(RED));
        Swatch::Second.set(&mut pulse, 0, MATRIX.leds, None);
        assert!(matches!(pulse, Effect::Blink(blink) if blink.from.rgb() == BLACK));
    }

    #[test]
    fn custom_colours_grow_up_to_the_led_count() {
        let mut effect = Kind::Custom.effect(Matrix { leds: 4, ..MATRIX });
        if let Effect::Custom(custom) = &mut effect {
            custom.colors.clear();
        }
        let green = Rgb {
            red: 0,
            green: 255,
            blue: 0,
        };
        Swatch::Led.set(&mut effect, 2, 4, Some(green));
        Swatch::Led.set(&mut effect, 4, 4, Some(green));
        let Effect::Custom(custom) = &effect else {
            panic!("the effect stays custom");
        };
        assert_eq!(custom.colors.len(), 3);
        assert_eq!(custom.colors[2], green);
        let capabilities = Capabilities {
            key_rows: 6,
            key_columns: 17,
            profiles: 4,
            linked_profiles: 0,
            layouts: heapless::Vec::new(),
            lightbar_leds: 4,
            host_signals: false,
            switch_models: 1,
            edition: None,
        };
        assert_eq!(effect.validate(&capabilities), Ok(()));
    }
}
