use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    component::{
        ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        color_picker::ColorPicker,
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        slider::Slider,
        switch::Switch,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::lighting::{
    Effect, HueRange, MAX_EFFECTS, RainbowWave, ReactiveColoring, ReactiveFade, Scanner,
    ScannerDirection,
};

use super::{
    LightingPage, WHITE,
    effect::{self, CATALOGUE, DIRECTIONS, Dial, HUE_RANGES, Paint},
    opaque_of, stepper,
};
use crate::panel;

const SLIDER_WIDTH: f32 = 280.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shift {
    Earlier,
    Later,
}

impl LightingPage {
    pub(super) fn effects_section(&self, cx: &Context<Self>) -> Div {
        let count = self
            .settings
            .as_ref()
            .map(|settings| settings.effects.len());
        panel::section(
            "Effects",
            Some(
                format!(
                    "Up to {MAX_EFFECTS} effect layers paint over the key colours, each over the \
                     ones above it. Effects pause while Tachyon mode is on and while a Fn layer \
                     with its own colours is shown."
                )
                .into(),
            ),
            cx,
        )
        .child(self.effect_list(cx))
        .child(h_flex().gap_2().items_center().child(self.catalogue_button(
            "add-effect",
            "Add effect",
            count.is_none_or(|count| count >= MAX_EFFECTS),
            false,
            cx,
        )))
        .when(count == Some(0), |this| {
            this.child(panel::note("No effect: the keys show their colours.", cx))
        })
        .when_some(self.selected_effect().copied(), |this, effect| {
            this.child(self.effect_editor(&effect, cx))
        })
    }

    fn effect_list(&self, cx: &Context<Self>) -> Div {
        let Some(settings) = &self.settings else {
            return div();
        };
        let theme = cx.theme();
        let count = settings.effects.len();
        v_flex()
            .gap_1()
            .children(settings.effects.iter().enumerate().map(|(index, effect)| {
                let selected = self.effect == Some(index);
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .px_3()
                    .py_1()
                    .rounded(theme.radius)
                    .border_1()
                    .border_color(if selected {
                        theme.primary
                    } else {
                        theme.border
                    })
                    .child(
                        h_flex()
                            .id(SharedString::from(format!("effect-{index}")))
                            .flex_1()
                            .min_w_0()
                            .gap_3()
                            .items_center()
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_effect(index, window, cx);
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{}", index + 1)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(effect::name(effect)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(effect::summary(effect)),
                            ),
                    )
                    .child(self.shift_button(index, Shift::Earlier, index == 0, cx))
                    .child(self.shift_button(index, Shift::Later, index + 1 == count, cx))
                    .child(
                        Button::new(SharedString::from(format!("effect-remove-{index}")))
                            .icon(IconName::Delete)
                            .xsmall()
                            .ghost()
                            .tooltip("Remove the effect")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.remove_effect(index, window, cx);
                            })),
                    )
            }))
    }

    fn shift_button(
        &self,
        index: usize,
        shift: Shift,
        blocked: bool,
        cx: &Context<Self>,
    ) -> Button {
        let (icon, name, tooltip) = match shift {
            Shift::Earlier => (IconName::ArrowUp, "up", "Move up"),
            Shift::Later => (IconName::ArrowDown, "down", "Move down"),
        };
        Button::new(SharedString::from(format!("effect-{name}-{index}")))
            .icon(icon)
            .xsmall()
            .ghost()
            .tooltip(tooltip)
            .disabled(blocked || self.settings.is_none())
            .on_click(cx.listener(move |this, _, window, cx| {
                this.shift_effect(index, shift, window, cx);
            }))
    }

    fn catalogue_button(
        &self,
        id: &'static str,
        label: &'static str,
        disabled: bool,
        replace: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        let current = self.selected_effect().copied();
        Button::new(id)
            .icon(IconName::Plus)
            .label(label)
            .small()
            .outline()
            .dropdown_caret(true)
            .disabled(disabled)
            .dropdown_menu(move |menu, _, _| {
                CATALOGUE.iter().fold(menu, |menu, entry| {
                    let page = page.clone();
                    let entry = *entry;
                    let checked = replace
                        && current
                            .as_ref()
                            .is_some_and(|current| effect::same_kind(current, &entry));
                    menu.item(
                        PopupMenuItem::new(effect::name(&entry))
                            .checked(checked)
                            .on_click(move |_, window, cx| {
                                page.update(cx, |page, cx| {
                                    if replace {
                                        page.replace_effect(entry, window, cx);
                                    } else {
                                        page.add_effect(entry, window, cx);
                                    }
                                })
                                .ok();
                            }),
                    )
                })
            })
    }

    fn effect_editor(&self, effect: &Effect, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        v_flex()
            .gap_2()
            .pt_3()
            .border_t_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .justify_between()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(effect::name(effect)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(effect::description(effect)),
                            ),
                    )
                    .child(self.catalogue_button(
                        "change-effect",
                        "Change effect",
                        false,
                        true,
                        cx,
                    )),
            )
            .children(self.effect_options(effect, cx))
            .children(self.effect_dials(effect, cx))
    }

    fn effect_options(&self, effect: &Effect, cx: &Context<Self>) -> Vec<Div> {
        let mut rows = Vec::new();
        if let Some(hues) = effect::hue_range(effect) {
            rows.push(panel::row(
                "Colours",
                None,
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .children(HUE_RANGES.map(|(range, label)| {
                        Button::new(SharedString::from(format!("hues-{label}")))
                            .label(label)
                            .xsmall()
                            .outline()
                            .selected(hues == range)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit_effect(|effect| set_hues(effect, range), window, cx);
                            }))
                    })),
                cx,
            ));
        }
        match effect {
            Effect::RainbowWave(wave) => rows.push(wave_direction(*wave, cx)),
            Effect::Scanner(scanner) => rows.extend(self.scanner_options(*scanner, cx)),
            Effect::KeyRipple(ripple) => {
                rows.push(toggle_row(
                    "ripple-palette",
                    "Random colours",
                    "Ripples take a random colour instead of dimming the key colours.",
                    ripple.palette,
                    |effect, enabled| {
                        if let Effect::KeyRipple(ripple) = effect {
                            ripple.palette = enabled;
                        }
                    },
                    cx,
                ));
                rows.push(toggle_row(
                    "ripple-inverted",
                    "Inverted",
                    "Ripples darken the keys instead of lighting them.",
                    ripple.inverted,
                    |effect, enabled| {
                        if let Effect::KeyRipple(ripple) = effect {
                            ripple.inverted = enabled;
                        }
                    },
                    cx,
                ));
            }
            Effect::ReactiveFade(fade) => rows.extend(trail_options(*fade, cx)),
            _ => {}
        }
        if let Some(enabled) = effect::analog_curve(effect) {
            rows.push(toggle_row(
                "analog-curve",
                "Use analog curve",
                "In gamepad mode the key depth follows the gamepad response curve; otherwise it \
                 is linear.",
                enabled,
                effect::set_analog_curve,
                cx,
            ));
        }
        rows.extend(self.color_rows(effect, cx));
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
        let unit = if across { "keys" } else { "rows" };
        let width = panel::row(
            "Width",
            Some("The size of the window, in keys across or rows down.".into()),
            stepper(
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
            ),
            cx,
        );
        let direction = panel::row(
            "Direction",
            Some("The direction it starts in; it turns at the edges.".into()),
            h_flex()
                .gap_1()
                .children(DIRECTIONS.map(|(direction, label)| {
                    Button::new(SharedString::from(format!("scanner-{label}")))
                        .label(label)
                        .xsmall()
                        .outline()
                        .selected(scanner.direction == direction)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.edit_effect(
                                |effect| {
                                    if let Effect::Scanner(scanner) = effect {
                                        scanner.direction = direction;
                                    }
                                },
                                window,
                                cx,
                            );
                        }))
                })),
            cx,
        );
        [width, direction]
    }

    fn color_rows(&self, effect: &Effect, cx: &Context<Self>) -> Vec<Div> {
        let (first, second) = effect::colors(effect);
        let first_label = if matches!(effect, Effect::ReactiveFade(_)) {
            "Colour"
        } else {
            "Primary colour"
        };
        let mut rows = Vec::new();
        if first.is_some() {
            rows.push(panel::row(
                first_label,
                None,
                ColorPicker::new(&self.first_color),
                cx,
            ));
        }
        if second.is_some() {
            rows.push(panel::row(
                "Secondary colour",
                None,
                ColorPicker::new(&self.second_color),
                cx,
            ));
        }
        rows
    }

    fn effect_dials(&self, effect: &Effect, cx: &Context<Self>) -> Vec<Div> {
        Dial::of(effect)
            .iter()
            .filter_map(|dial| {
                let slider = self
                    .dials
                    .iter()
                    .find(|(other, _)| other == dial)
                    .map(|(_, slider)| slider)?;
                let reading = dial.reading(effect).unwrap_or_default();
                let description = match dial.detail(effect) {
                    Some(detail) => format!("{reading}. {detail}"),
                    None => reading,
                };
                Some(panel::row(
                    dial.label(effect),
                    Some(description.into()),
                    div().w(px(SLIDER_WIDTH)).child(Slider::new(slider)),
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

fn wave_direction(wave: RainbowWave, cx: &Context<LightingPage>) -> Div {
    panel::row(
        "Direction",
        None,
        h_flex()
            .gap_1()
            .children([(false, "Left to right"), (true, "Right to left")].map(
                |(forward, label)| {
                    Button::new(SharedString::from(format!("wave-{label}")))
                        .label(label)
                        .xsmall()
                        .outline()
                        .selected(wave.forward == forward)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.edit_effect(
                                |effect| {
                                    if let Effect::RainbowWave(wave) = effect {
                                        wave.forward = forward;
                                    }
                                },
                                window,
                                cx,
                            );
                        }))
                },
            )),
        cx,
    )
}

fn toggle_row(
    id: &'static str,
    label: &'static str,
    description: &'static str,
    checked: bool,
    change: fn(&mut Effect, bool),
    cx: &Context<LightingPage>,
) -> Div {
    panel::row(
        label,
        Some(description.into()),
        Switch::new(id).checked(checked).on_change(cx.listener(
            move |this, enabled: &bool, window, cx| {
                let enabled = *enabled;
                this.edit_effect(|effect| change(effect, enabled), window, cx);
            },
        )),
        cx,
    )
}

fn trail_options(fade: ReactiveFade, cx: &Context<LightingPage>) -> [Div; 2] {
    let paint = effect::paint_of(fade.coloring);
    let modes = [
        (Paint::KeyColor, "Base layer"),
        (Paint::Palette, "Random colours"),
        (Paint::Fixed, "Single colour"),
    ];
    let coloring = panel::row(
        "Colour mode",
        Some("The colour a pressed key lights up in.".into()),
        h_flex().gap_1().children(modes.map(|(mode, label)| {
            Button::new(SharedString::from(format!("trail-{label}")))
                .label(label)
                .xsmall()
                .outline()
                .selected(paint == mode)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.choose_trail_paint(mode, window, cx);
                }))
        })),
        cx,
    );
    let inverted = toggle_row(
        "trail-inverted",
        "Inverted",
        "Pressed keys go dark and light up again instead.",
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
