use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, Sizable as _, h_flex, input::Input, tooltip::Tooltip, v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::lightbar::{
    Builtin, Comparator, Effect, Entry, MAX_ENTRIES, Range, Signal, Solid, Target, Term, Title,
};

use super::{
    LightbarPage,
    entry::{DEFAULT_TIME_MS, Kind, Matrix, color, compare, full_range, meter, single, title},
};
use crate::{
    page::lighting::{ICON_SIZE, LABEL_TEXT},
    panel, theme,
};

const KEY_DEPTH_THRESHOLD: f32 = 5.0;
const CAPS_LOCK_LEDS: u8 = 2;
const ROW_HEIGHT: f32 = 44.0;
const ROW_RADIUS: f32 = 6.0;
const ROW_LINE: f32 = 16.8;
const SEARCH_ICON: f32 = 16.0;
const INTRO_TEXT: f32 = 16.0;
const INTRO_LINE: f32 = 22.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Category {
    Visual,
    General,
    KeyPress,
}

pub(super) struct Preset {
    pub(super) name: &'static str,
    icon: Catalog,
    description: &'static str,
    category: Category,
    build: fn(Matrix) -> Entry,
}

pub(super) const PRESETS: [Preset; 8] = [
    Preset {
        name: "Solid",
        icon: Catalog::PaintBucket,
        description: "Set your Light Indicator to a single color or a color per LED.",
        category: Category::Visual,
        build: |matrix| plain(Kind::Solid.effect(matrix)),
    },
    Preset {
        name: "Pulse",
        icon: Catalog::HeartPulse,
        description: "Fade one color in and out or between two colors.",
        category: Category::Visual,
        build: |matrix| plain(Kind::Blink.effect(matrix)),
    },
    Preset {
        name: "Sync with Keyboard",
        icon: Catalog::RefreshCw,
        description: "This effect matches the Light Indicator with the RGB effects on the keyboard.",
        category: Category::Visual,
        build: |matrix| plain(Kind::Mirror.effect(matrix)),
    },
    Preset {
        name: "Profile Indicator",
        icon: Catalog::ToggleRight,
        description: "This effect displays the color and slot of the active profile.",
        category: Category::General,
        build: |matrix| plain(Kind::ProfileIndicator.effect(matrix)),
    },
    Preset {
        name: "Caps Lock Indicator",
        icon: Catalog::Keyboard,
        description: "This effect displays the color of your choice to indicate whether Caps Lock is enabled.",
        category: Category::General,
        build: caps_lock,
    },
    Preset {
        name: "Brightness Change",
        icon: Catalog::Sun,
        description: "This effect displays the keyboard's brightness level whenever it is being adjusted.",
        category: Category::General,
        build: brightness,
    },
    Preset {
        name: "Key Depth",
        icon: Catalog::ArrowDownToLine,
        description: "This effect visually represents your key press depth with a progress bar.",
        category: Category::KeyPress,
        build: key_depth,
    },
    Preset {
        name: "Typing Speed Meter (KPS)",
        icon: Catalog::Gauge,
        description: "This effect displays your typing speed.",
        category: Category::KeyPress,
        build: typing_speed,
    },
];

impl Category {
    const ALL: [Self; 3] = [Self::Visual, Self::General, Self::KeyPress];

    const fn name(self) -> &'static str {
        match self {
            Self::Visual => "VISUAL",
            Self::General => "GENERAL INDICATORS",
            Self::KeyPress => "KEY PRESS INDICATORS",
        }
    }
}

impl Preset {
    pub(super) fn entry(&self, matrix: Matrix) -> Entry {
        let mut entry = (self.build)(matrix);
        entry.title = title(self.name);
        entry
    }

    fn matches(&self, search: &str) -> bool {
        let search = search.trim().to_lowercase();
        search.is_empty()
            || self.name.to_lowercase().contains(&search)
            || self.description.to_lowercase().contains(&search)
    }
}

impl LightbarPage {
    pub(super) fn presets_card(&self, cx: &Context<Self>) -> Option<Div> {
        self.settings.as_ref()?;
        let theme = cx.theme();
        let search = self.search.read(cx).value();
        let full = self
            .entries(self.owner)
            .is_none_or(|entries| entries.len() >= MAX_ENTRIES);
        let sections = Category::ALL.into_iter().filter_map(|category| {
            let rows: Vec<_> = PRESETS
                .iter()
                .enumerate()
                .filter(|(_, preset)| preset.category == category && preset.matches(&search))
                .map(|(index, preset)| Self::preset_row(index, preset, full, cx))
                .collect();
            (!rows.is_empty()).then(|| {
                v_flex()
                    .w_full()
                    .gap_3()
                    .child(panel::label(category.name(), cx))
                    .child(v_flex().w_full().gap_2().children(rows))
            })
        });
        let intro = v_flex()
            .w_full()
            .gap_4()
            .child(
                div()
                    .text_size(px(INTRO_TEXT))
                    .line_height(px(INTRO_LINE))
                    .text_color(theme.group_box_foreground)
                    .child(if full {
                        SharedString::from(format!("Up to {MAX_ENTRIES} effects"))
                    } else {
                        SharedString::from(
                            "To add a new effect, simply click on one from the list below.",
                        )
                    }),
            )
            .child(
                Input::new(&self.search)
                    .large()
                    .bg(theme::palette().input)
                    .prefix(Icon::new(Catalog::Search).size(px(SEARCH_ICON))),
            );
        Some(
            panel::section("Effect Presets", None, cx).p_6().child(
                v_flex()
                    .w_full()
                    .gap_6()
                    .child(intro)
                    .child(v_flex().w_full().gap_4().children(sections)),
            ),
        )
    }

    fn preset_row(
        index: usize,
        preset: &Preset,
        full: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let description = preset.description;
        let primary = theme.primary;
        h_flex()
            .id(SharedString::from(format!("lightbar-preset-{index}")))
            .w_full()
            .h(px(ROW_HEIGHT))
            .px_4()
            .gap_2()
            .items_center()
            .rounded(px(ROW_RADIUS))
            .border_1()
            .border_color(theme.transparent)
            .bg(theme.accent)
            .text_size(px(LABEL_TEXT))
            .line_height(px(ROW_LINE))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.group_box_foreground)
            .child(
                div()
                    .text_color(theme.foreground)
                    .child(Icon::new(preset.icon).size(px(ICON_SIZE))),
            )
            .child(div().min_w_0().truncate().child(preset.name))
            .tooltip(move |window, cx| Tooltip::new(description).build(window, cx))
            .map(|this| {
                if full {
                    this.opacity(0.5)
                } else {
                    this.cursor_pointer()
                        .hover(move |style| style.border_color(primary))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(preset) = PRESETS.get(index) {
                                let entry = preset.entry(this.matrix(cx));
                                this.add_entry(this.owner, entry, window, cx);
                            }
                        }))
                }
            })
    }
}

const fn plain(effect: Effect) -> Entry {
    Entry {
        title: Title::new(),
        condition: None,
        effect,
    }
}

fn caps_lock(matrix: Matrix) -> Entry {
    let last = matrix.leds.saturating_sub(1);
    Entry {
        title: Title::new(),
        condition: Some(single(compare(
            Signal::Builtin(Builtin::CapsLock),
            Comparator::Equal,
            1.0,
        ))),
        effect: Effect::Solid(Solid {
            color: color(255, 255, 255),
            target: Target::Lightbar(Range {
                first: matrix.leds.saturating_sub(CAPS_LOCK_LEDS),
                last,
            }),
        }),
    }
}

fn brightness(matrix: Matrix) -> Entry {
    Entry {
        title: Title::new(),
        condition: Some(single(Term::Trigger {
            signal: Signal::Builtin(Builtin::Brightness),
            duration_ms: DEFAULT_TIME_MS,
        })),
        effect: Effect::Meter(meter(
            Builtin::Brightness,
            color(255, 255, 255),
            None,
            full_range(matrix.leds),
        )),
    }
}

fn key_depth(matrix: Matrix) -> Entry {
    Entry {
        title: Title::new(),
        condition: Some(single(compare(
            Signal::Builtin(Builtin::AnalogLevel),
            Comparator::Greater,
            KEY_DEPTH_THRESHOLD,
        ))),
        effect: Effect::Meter(meter(
            Builtin::AnalogLevel,
            color(255, 0, 0),
            None,
            full_range(matrix.leds),
        )),
    }
}

fn typing_speed(matrix: Matrix) -> Entry {
    plain(Effect::Meter(meter(
        Builtin::KeyRate,
        color(255, 0, 0),
        Some(color(0, 255, 0)),
        full_range(matrix.leds),
    )))
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        configuration::{Capabilities, Validate as _},
        heapless,
        lightbar::{Entries, Meter},
    };

    use super::*;
    use crate::page::lightbar::entry::maximum_of;

    fn capabilities(leds: u8) -> Capabilities {
        Capabilities {
            key_rows: 6,
            key_columns: 17,
            profiles: 4,
            linked_profiles: 0,
            layouts: heapless::Vec::new(),
            lightbar_leds: leds,
            host_signals: false,
            switch_models: 1,
            edition: None,
        }
    }

    #[test]
    fn every_preset_is_valid_on_every_light_bar() {
        for leds in [1, 2, 10, 16] {
            let matrix = Matrix {
                leds,
                rows: 6,
                columns: 17,
            };
            for preset in &PRESETS {
                let entry = preset.entry(matrix);
                assert_eq!(
                    entry.validate(&capabilities(leds)),
                    Ok(()),
                    "{}",
                    preset.name
                );
            }
        }
    }

    #[test]
    fn the_caps_lock_indicator_uses_the_last_two_leds() {
        let entry = caps_lock(Matrix {
            leds: 10,
            rows: 6,
            columns: 17,
        });
        assert!(matches!(
            entry.effect,
            Effect::Solid(Solid {
                target: Target::Lightbar(Range { first: 8, last: 9 }),
                ..
            })
        ));
    }

    #[test]
    fn the_meters_follow_their_signal_scale() {
        let matrix = Matrix {
            leds: 10,
            rows: 6,
            columns: 17,
        };
        let maxima: Vec<f32> = [brightness(matrix), key_depth(matrix), typing_speed(matrix)]
            .iter()
            .filter_map(|entry| match &entry.effect {
                Effect::Meter(Meter { maximum, .. }) => Some(*maximum),
                _ => None,
            })
            .collect();
        assert_eq!(maxima, [100.0, 255.0, 30.0]);
        let entries: Entries = [brightness(matrix), key_depth(matrix)]
            .into_iter()
            .collect();
        assert_eq!(entries.validate(&capabilities(10)), Ok(()));
        assert!(maximum_of(Signal::Builtin(Builtin::KeyRate)) > 0.0);
    }
}
