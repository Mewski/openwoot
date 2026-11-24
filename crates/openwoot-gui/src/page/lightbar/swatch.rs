use openwoot_protocol::{
    lightbar::{Background, Effect, MAX_LEDS},
    lighting::{Color, Rgb},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Swatch {
    Main,
    Second,
    Third,
}

impl Swatch {
    pub(super) const ALL: [Self; 3] = [Self::Main, Self::Second, Self::Third];

    pub(super) fn color(self, effect: &Effect, led: u8) -> Option<Color> {
        match (self, effect) {
            (Self::Main, Effect::Meter(meter)) => Some(meter.color),
            (Self::Second, Effect::Meter(meter)) => meter.background,
            (Self::Third, Effect::Meter(meter)) => meter.gradient_start,
            (Self::Main, Effect::ProfileIndicator(indicator)) => match indicator.background {
                Background::Fixed(color) => Some(color),
                _ => None,
            },
            (Self::Main, Effect::Blink(blink)) => Some(blink.from),
            (Self::Second, Effect::Blink(blink)) => Some(blink.to),
            (Self::Main, Effect::Solid(solid)) => Some(solid.color),
            (Self::Main, Effect::Custom(custom)) => custom
                .colors
                .get(usize::from(led))
                .map(|color| color.opaque()),
            _ => None,
        }
    }

    pub(super) fn set(self, effect: &mut Effect, led: u8, leds: u8, color: Color) {
        match (self, effect) {
            (Self::Main, Effect::Meter(meter)) => meter.color = color,
            (Self::Second, Effect::Meter(meter)) => meter.background = Some(color),
            (Self::Third, Effect::Meter(meter)) => meter.gradient_start = Some(color),
            (Self::Main, Effect::ProfileIndicator(indicator)) => {
                indicator.background = Background::Fixed(color);
            }
            (Self::Main, Effect::Blink(blink)) => blink.from = color,
            (Self::Second, Effect::Blink(blink)) => blink.to = color,
            (Self::Main, Effect::Solid(solid)) => solid.color = color,
            (Self::Main, Effect::Custom(custom)) => {
                let wanted = usize::from(led) + 1;
                if wanted > usize::from(leds).min(MAX_LEDS) {
                    return;
                }
                while custom.colors.len() < wanted {
                    if custom.colors.push(Rgb::default()).is_err() {
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
    use crate::page::lightbar::entry::{KINDS, Kind, Matrix, color};

    const MATRIX: Matrix = Matrix {
        leds: 10,
        rows: 6,
        columns: 17,
    };

    #[test]
    fn a_swatch_writes_what_it_reads() {
        let red = color(255, 0, 0);
        for kind in KINDS {
            let mut effect = kind.effect(MATRIX);
            for swatch in Swatch::ALL {
                let before = swatch.color(&effect, 3);
                swatch.set(&mut effect, 3, MATRIX.leds, red);
                if before.is_some() {
                    assert_eq!(swatch.color(&effect, 3), Some(red), "{kind:?} {swatch:?}");
                }
            }
        }
    }

    #[test]
    fn custom_colours_grow_up_to_the_led_count() {
        let mut effect = Kind::Custom.effect(Matrix { leds: 4, ..MATRIX });
        if let Effect::Custom(custom) = &mut effect {
            custom.colors.clear();
        }
        let green = color(0, 255, 0);
        Swatch::Main.set(&mut effect, 2, 4, green);
        Swatch::Main.set(&mut effect, 4, 4, green);
        let Effect::Custom(custom) = &effect else {
            panic!("the effect stays custom");
        };
        assert_eq!(custom.colors.len(), 3);
        assert_eq!(custom.colors[2], green.rgb());
        let capabilities = Capabilities {
            key_rows: 6,
            key_columns: 17,
            profiles: 4,
            layouts: heapless::Vec::new(),
            lightbar_leds: 4,
            host_signals: false,
            switch_models: 1,
            edition: None,
        };
        assert_eq!(effect.validate(&capabilities), Ok(()));
    }
}
