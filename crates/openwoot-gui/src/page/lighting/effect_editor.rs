use gpui_kit::{
    Context, Div, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window,
    component::{
        IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        color_picker::ColorPicker,
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        switch::Switch,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::lighting::{
    Effect, HueRange, MAX_EFFECTS, ReactiveColoring, ReactiveFade, Scanner, ScannerDirection,
};

use super::{
    LightingPage, WHITE, add_button,
    effect::{self, CATALOGUE, Dial, HUE_RANGES, Paint},
    opaque_of, pill, segmented, select, stepper, sunken, tile, wide_slider,
};
use crate::panel;

const LAYERS_WIDTH: f32 = 180.0;
const WIDE_CONTENT: f32 = 1100.0;
const WAVE_DIRECTIONS: [(bool, &str); 2] = [(true, "←"), (false, "→")];
const SCAN_DIRECTIONS: [(ScannerDirection, &str); 4] = [
    (ScannerDirection::Left, "←"),
    (ScannerDirection::Right, "→"),
    (ScannerDirection::Up, "↑"),
    (ScannerDirection::Down, "↓"),
];
const TRAIL_PAINTS: [(Paint, &str); 3] = [
    (Paint::KeyColor, "Base"),
    (Paint::Palette, "Random"),
    (Paint::Fixed, "Single"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shift {
    Earlier,
    Later,
}

impl LightingPage {
    pub(super) fn effects_card(&self, window: &Window, cx: &Context<Self>) -> Option<Div> {
        let settings = self.settings.as_ref()?;
        let brightness = self.brightness.read(cx).value().end();
        let wide = f32::from(panel::content_width(window)) >= WIDE_CONTENT;
        Some(
            panel::section(
                "Effects",
                Some("Paused in Tachyon mode and while an Fn layer has its own colours.".into()),
                cx,
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_6()
                    .items_start()
                    .child(div().flex_1().min_w_0().child(panel::field(
                        "Presets",
                        Self::preset_menu(cx),
                        cx,
                    )))
                    .child(div().flex_1().min_w_0().child(panel::field(
                        "Brightness",
                        wide_slider(&self.brightness, format!("{brightness:.0} %"), cx),
                        cx,
                    ))),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_6()
                    .items_start()
                    .child(div().flex_none().w(px(LAYERS_WIDTH)).child(panel::field(
                        "Effect layers",
                        self.effect_layers(&settings.effects, cx),
                        cx,
                    )))
                    .child(self.effect_details(wide, cx)),
            ),
        )
    }

    fn effect_layers(&self, effects: &[Effect], cx: &Context<Self>) -> Div {
        let count = effects.len();
        v_flex()
            .w_full()
            .gap_2()
            .children(effects.iter().enumerate().map(|(index, effect)| {
                pill(
                    format!("effect-{index}"),
                    effect::name(effect),
                    None,
                    self.effect == Some(index),
                    false,
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.select_effect(index, window, cx);
                }))
                .child(Self::effect_options_menu(index, count, cx))
            }))
            .when(count < MAX_EFFECTS, |this| {
                this.child(
                    add_button("add-effect", "Add effect", cx).on_click(cx.listener(
                        |this, _, window, cx| {
                            if let Some(effect) = CATALOGUE.first() {
                                this.add_effect(*effect, window, cx);
                            }
                        },
                    )),
                )
            })
    }

    fn effect_details(&self, wide: bool, cx: &Context<Self>) -> Div {
        let Some(effect) = self.selected_effect().copied() else {
            return div().flex_1().min_w_0().child(panel::field(
                "Effect parameters",
                sunken(cx)
                    .py_4()
                    .items_center()
                    .child(panel::note("Select an effect to configure it", cx)),
                cx,
            ));
        };
        let selection = panel::field("Effect selection", Self::effect_tiles(&effect, cx), cx);
        let parameters = panel::field("Effect parameters", self.effect_parameters(&effect, cx), cx);
        if wide {
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_6()
                .items_start()
                .child(div().flex_1().min_w_0().child(selection))
                .child(div().flex_1().min_w_0().child(parameters))
        } else {
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_4()
                .child(selection)
                .child(parameters)
        }
    }

    fn effect_tiles(current: &Effect, cx: &Context<Self>) -> Div {
        sunken(cx).child(div().grid().grid_cols(2).gap_1().children(
            CATALOGUE.iter().enumerate().map(|(index, entry)| {
                let entry = *entry;
                tile(
                    format!("effect-kind-{index}"),
                    effect::icon(&entry),
                    effect::name(&entry),
                    effect::description(&entry),
                    effect::same_kind(current, &entry),
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    let changed = this
                        .selected_effect()
                        .is_some_and(|effect| !effect::same_kind(effect, &entry));
                    if changed {
                        this.replace_effect(entry, window, cx);
                    }
                }))
            }),
        ))
    }

    fn effect_parameters(&self, effect: &Effect, cx: &Context<Self>) -> Div {
        let rows: Vec<Div> = self
            .effect_options(effect, cx)
            .into_iter()
            .chain(self.effect_dials(effect, cx))
            .collect();
        let body = sunken(cx).p_3().gap_3();
        if rows.is_empty() {
            body.child(panel::note("No parameters", cx))
        } else {
            body.children(rows)
        }
    }

    fn effect_options_menu(
        index: usize,
        count: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        Button::new(SharedString::from(format!("effect-options-{index}")))
            .icon(IconName::EllipsisVertical)
            .ghost()
            .xsmall()
            .dropdown_menu(move |menu, _, _| {
                let (up, down, delete) = (page.clone(), page.clone(), page.clone());
                menu.item(PopupMenuItem::new("Move up").disabled(index == 0).on_click(
                    move |_, window, cx| {
                        up.update(cx, |page, cx| {
                            page.shift_effect(index, Shift::Earlier, window, cx);
                        })
                        .ok();
                    },
                ))
                .item(
                    PopupMenuItem::new("Move down")
                        .disabled(index + 1 >= count)
                        .on_click(move |_, window, cx| {
                            down.update(cx, |page, cx| {
                                page.shift_effect(index, Shift::Later, window, cx);
                            })
                            .ok();
                        }),
                )
                .separator()
                .item(PopupMenuItem::new("Delete").on_click(move |_, window, cx| {
                    delete
                        .update(cx, |page, cx| page.remove_effect(index, window, cx))
                        .ok();
                }))
            })
    }

    fn hue_menu(current: HueRange, cx: &Context<Self>) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        select("effect-hues", effect::hue_name(current)).dropdown_menu(move |menu, _, _| {
            HUE_RANGES.iter().fold(menu, |menu, (range, label)| {
                let page = page.clone();
                let range = *range;
                menu.item(
                    PopupMenuItem::new(*label)
                        .checked(range == current)
                        .on_click(move |_, window, cx| {
                            page.update(cx, |page, cx| {
                                page.edit_effect(|effect| set_hues(effect, range), window, cx);
                            })
                            .ok();
                        }),
                )
            })
        })
    }

    fn effect_options(&self, effect: &Effect, cx: &Context<Self>) -> Vec<Div> {
        let mut rows = Vec::new();
        if let Some(hues) = effect::hue_range(effect) {
            rows.push(panel::field("Colours", Self::hue_menu(hues, cx), cx));
        }
        match effect {
            Effect::RainbowWave(wave) => rows.push(choice_field(
                "Direction",
                "wave-direction",
                &WAVE_DIRECTIONS,
                wave.forward,
                |effect, forward| {
                    if let Effect::RainbowWave(wave) = effect {
                        wave.forward = forward;
                    }
                },
                cx,
            )),
            Effect::Scanner(scanner) => rows.extend(self.scanner_options(*scanner, cx)),
            Effect::KeyRipple(ripple) => {
                rows.push(toggle_line(
                    "ripple-palette",
                    "Random colours",
                    None,
                    ripple.palette,
                    |effect, enabled| {
                        if let Effect::KeyRipple(ripple) = effect {
                            ripple.palette = enabled;
                        }
                    },
                    cx,
                ));
                rows.push(toggle_line(
                    "ripple-inverted",
                    "Inverted",
                    Some("Ripples darken the keys."),
                    ripple.inverted,
                    |effect, enabled| {
                        if let Effect::KeyRipple(ripple) = effect {
                            ripple.inverted = enabled;
                        }
                    },
                    cx,
                ));
            }
            Effect::ReactiveFade(fade) => rows.extend(self.trail_options(*fade, cx)),
            Effect::SimplexNoise(_) | Effect::PolarGradient(_) => rows.push(panel::field(
                "Colours",
                h_flex()
                    .gap_2()
                    .child(ColorPicker::new(&self.first_color).small())
                    .child(ColorPicker::new(&self.second_color).small()),
                cx,
            )),
            _ => {}
        }
        if let Some(enabled) = effect::analog_curve(effect) {
            rows.push(toggle_line(
                "analog-curve",
                "Analog curve",
                Some("Follow the gamepad response curve in gamepad mode."),
                enabled,
                effect::set_analog_curve,
                cx,
            ));
        }
        rows
    }

    fn scanner_options(&self, scanner: Scanner, cx: &Context<Self>) -> [Div; 2] {
        let across = matches!(
            scanner.direction,
            ScannerDirection::Left | ScannerDirection::Right
        );
        let limit = self
            .store
            .read(cx)
            .keyboard()
            .map_or(u8::MAX, |keyboard| {
                if across {
                    keyboard.capabilities.key_columns
                } else {
                    keyboard.capabilities.key_rows
                }
            })
            .max(1);
        let unit = match (across, scanner.width) {
            (true, 1) => "key",
            (true, _) => "keys",
            (false, 1) => "row",
            (false, _) => "rows",
        };
        let width = panel::field(
            "Width",
            h_flex().child(stepper(
                "scanner-width",
                scanner.width,
                1..=limit,
                format!("{} {unit}", scanner.width),
                cx.listener(|this, width: &u8, window, cx| {
                    let width = *width;
                    this.edit_effect(
                        |effect| {
                            if let Effect::Scanner(scanner) = effect {
                                scanner.width = width;
                            }
                        },
                        window,
                        cx,
                    );
                }),
            )),
            cx,
        );
        let direction = choice_field(
            "Direction",
            "scanner-direction",
            &SCAN_DIRECTIONS,
            scanner.direction,
            |effect, direction| {
                if let Effect::Scanner(scanner) = effect {
                    scanner.direction = direction;
                }
            },
            cx,
        );
        [width, direction]
    }

    fn trail_options(&self, fade: ReactiveFade, cx: &Context<Self>) -> [Div; 2] {
        let paint = effect::paint_of(fade.coloring);
        let selected = TRAIL_PAINTS
            .iter()
            .position(|(mode, _)| *mode == paint)
            .unwrap_or(0);
        let coloring = panel::field(
            "Colour",
            h_flex()
                .gap_2()
                .items_center()
                .child(segmented(
                    "trail-colour",
                    TRAIL_PAINTS.map(|(_, label)| label),
                    selected,
                    cx.listener(|this, index: &usize, window, cx| {
                        if let Some((mode, _)) = TRAIL_PAINTS.get(*index) {
                            this.choose_trail_paint(*mode, window, cx);
                        }
                    }),
                ))
                .when(paint == Paint::Fixed, |this| {
                    this.child(ColorPicker::new(&self.first_color).small())
                }),
            cx,
        );
        let inverted = toggle_line(
            "trail-inverted",
            "Inverted",
            Some("Pressed keys go dark and light up again."),
            fade.inverted,
            |effect, enabled| {
                if let Effect::ReactiveFade(fade) = effect {
                    fade.inverted = enabled;
                }
            },
            cx,
        );
        [coloring, inverted]
    }

    fn effect_dials(&self, effect: &Effect, cx: &Context<Self>) -> Vec<Div> {
        Dial::of(effect)
            .iter()
            .filter_map(|dial| {
                let state = self
                    .dials
                    .iter()
                    .find(|(other, _)| other == dial)
                    .map(|(_, state)| state)?;
                let mut shown = *effect;
                dial.apply(&mut shown, state.read(cx).value().end());
                Some(panel::field(
                    dial.label(effect),
                    wide_slider(state, dial.reading(&shown).unwrap_or_default(), cx),
                    cx,
                ))
            })
            .collect()
    }

    fn choose_trail_paint(&mut self, mode: Paint, window: &mut Window, cx: &mut Context<Self>) {
        let fixed = self
            .first_color
            .read(cx)
            .value()
            .map_or(WHITE.opaque(), opaque_of);
        let coloring = match mode {
            Paint::KeyColor => ReactiveColoring::KeyColor,
            Paint::Palette => ReactiveColoring::Palette,
            Paint::Fixed => ReactiveColoring::Fixed(fixed),
        };
        self.edit_effect(
            |effect| {
                if let Effect::ReactiveFade(fade) = effect {
                    fade.coloring = coloring;
                }
            },
            window,
            cx,
        );
        self.sync_effect_controls(window, cx);
    }

    fn select_effect(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.effect = Some(index);
        self.sync_effect_controls(window, cx);
        cx.notify();
    }

    fn add_effect(&mut self, effect: Effect, window: &mut Window, cx: &mut Context<Self>) {
        let Some(count) = self
            .settings
            .as_ref()
            .map(|settings| settings.effects.len())
        else {
            return;
        };
        if count >= MAX_EFFECTS {
            return;
        }
        self.edit_settings(
            |settings| {
                settings.effects.push(effect).ok();
            },
            window,
            cx,
        );
        self.select_effect(count, window, cx);
    }

    fn replace_effect(&mut self, effect: Effect, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_effect(|current| *current = effect, window, cx);
        self.sync_effect_controls(window, cx);
    }

    fn remove_effect(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(count) = self
            .settings
            .as_ref()
            .map(|settings| settings.effects.len())
        else {
            return;
        };
        if index >= count {
            return;
        }
        self.edit_settings(
            |settings| {
                settings.effects.remove(index);
            },
            window,
            cx,
        );
        self.effect = match (self.effect, count - 1) {
            (_, 0) => None,
            (Some(selected), remaining) if selected > index || selected >= remaining => {
                Some(selected.saturating_sub(1))
            }
            (selected, _) => selected,
        };
        self.sync_effect_controls(window, cx);
    }

    fn shift_effect(
        &mut self,
        index: usize,
        shift: Shift,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(count) = self
            .settings
            .as_ref()
            .map(|settings| settings.effects.len())
        else {
            return;
        };
        let target = match shift {
            Shift::Earlier => index.checked_sub(1),
            Shift::Later => Some(index + 1).filter(|target| *target < count),
        };
        let Some(target) = target.filter(|_| index < count) else {
            return;
        };
        self.edit_settings(|settings| settings.effects.swap(index, target), window, cx);
        self.effect = self.effect.map(|selected| match selected {
            selected if selected == index => target,
            selected if selected == target => index,
            selected => selected,
        });
    }
}

const fn set_hues(effect: &mut Effect, hues: HueRange) {
    match effect {
        Effect::RainbowWave(wave) => wave.hues = hues,
        Effect::HueCycle(cycle) => cycle.hues = hues,
        _ => {}
    }
}

fn choice_field<T: Copy + PartialEq + 'static, const N: usize>(
    label: &'static str,
    id: &'static str,
    choices: &'static [(T, &'static str); N],
    current: T,
    change: fn(&mut Effect, T),
    cx: &Context<LightingPage>,
) -> Div {
    let selected = choices
        .iter()
        .position(|(value, _)| *value == current)
        .unwrap_or(0);
    panel::field(
        label,
        h_flex().child(segmented(
            id,
            choices.iter().map(|(_, name)| *name),
            selected,
            cx.listener(move |this, index: &usize, window, cx| {
                if let Some((value, _)) = choices.get(*index) {
                    let value = *value;
                    this.edit_effect(|effect| change(effect, value), window, cx);
                }
            }),
        )),
        cx,
    )
}

fn toggle_line(
    id: &'static str,
    label: &'static str,
    help: Option<&'static str>,
    checked: bool,
    change: fn(&mut Effect, bool),
    cx: &Context<LightingPage>,
) -> Div {
    panel::toggle(
        label,
        help.map(SharedString::from),
        Switch::new(id).checked(checked).on_change(cx.listener(
            move |this, enabled: &bool, window, cx| {
                let enabled = *enabled;
                this.edit_effect(|effect| change(effect, enabled), window, cx);
            },
        )),
        cx,
    )
}
