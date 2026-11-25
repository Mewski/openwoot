use gpui_kit::{
    AnyElement, Context, Div, IntoElement, ParentElement as _, SharedString, Styled as _, Window,
    component::{h_flex, v_flex},
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::lighting::{
    Effect, KeyRipple, ReactiveColoring, ReactiveFade, ScannerDirection,
};

use super::{
    Choice, LightingPage, choices,
    controls::param_label,
    dial_row,
    effect::{self, Dial, HUE_RANGES, Paint, Turn},
    sunken,
};
use crate::{
    color_picker::Swatch,
    dropdown::{Dropdown, Item},
    panel,
    switch::Switch,
};

const SWATCH_SIZE: f32 = 30.0;
const WAVE_DIRECTIONS: [(bool, &str); 2] = [(true, "Left"), (false, "Right")];
const SCAN_DIRECTIONS: [(ScannerDirection, &str); 4] = [
    (ScannerDirection::Left, "Left"),
    (ScannerDirection::Right, "Right"),
    (ScannerDirection::Up, "Up"),
    (ScannerDirection::Down, "Down"),
];
const TRAIL_PAINTS: [(Paint, &str); 3] = [
    (Paint::KeyColor, "Base layer"),
    (Paint::Palette, "Random colours"),
    (Paint::Fixed, "Single colour"),
];
const FADE_DIRECTIONS: [(bool, &str); 2] = [(false, "Normal"), (true, "Inverted")];

impl LightingPage {
    pub(super) fn effect_parameters(&self, cx: &Context<Self>) -> Div {
        let body = sunken(cx).p_4().gap_4();
        let Some(effect) = self.focused_effect().copied() else {
            return body.child(panel::note("Select an effect to configure it", cx));
        };
        let rows = self.parameter_rows(&effect, cx);
        if rows.is_empty() {
            body.child(panel::note("This effect has no parameters", cx))
        } else {
            body.children(rows)
        }
    }

    fn parameter_rows(&self, effect: &Effect, cx: &Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        if let Some(hues) = effect::hue_range(effect) {
            rows.push(Self::look_menu(effect, hues, cx).into_any_element());
        }
        rows.extend(
            self.dial_rows(effect, cx)
                .into_iter()
                .map(IntoElement::into_any_element),
        );
        match effect {
            Effect::RainbowWave(wave) => rows.push(
                choice_row(
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
                )
                .into_any_element(),
            ),
            Effect::Scanner(scanner) => rows.push(
                choice_row(
                    "Direction",
                    "scan-direction",
                    &SCAN_DIRECTIONS,
                    scanner.direction,
                    |effect, direction| {
                        if let Effect::Scanner(scanner) = effect {
                            scanner.direction = direction;
                        }
                    },
                    cx,
                )
                .into_any_element(),
            ),
            Effect::KeyRipple(ripple) => rows.extend(Self::ripple_rows(*ripple, cx)),
            Effect::ReactiveFade(fade) => rows.extend(self.trail_rows(*fade, cx)),
            Effect::PolarGradient(_) => {
                for (turn, label) in [
                    (Turn::Spin, "Spin clockwise"),
                    (Turn::Twist, "Spiral clockwise"),
                ] {
                    let checked = effect::clockwise(effect, turn).unwrap_or_default();
                    rows.push(
                        switch_row(
                            label,
                            None,
                            checked,
                            move |effect, clockwise| effect::set_clockwise(effect, turn, clockwise),
                            cx,
                        )
                        .into_any_element(),
                    );
                }
                rows.extend(self.color_rows(cx));
            }
            Effect::SimplexNoise(_) => rows.extend(self.color_rows(cx)),
            _ => {}
        }
        if let Some(enabled) = effect::analog_curve(effect) {
            rows.push(
                switch_row(
                    "Use analog curve",
                    Some("Follow the gamepad analog curve instead of a linear response"),
                    enabled,
                    effect::set_analog_curve,
                    cx,
                )
                .into_any_element(),
            );
        }
        rows
    }

    fn dial_rows(&self, effect: &Effect, cx: &Context<Self>) -> Vec<Div> {
        Dial::of(effect)
            .iter()
            .filter_map(|dial| {
                let control = self.dial(*dial)?;
                Some(dial_row(
                    dial.label(effect),
                    &control.slider,
                    &control.input,
                    dial.unit(),
                    cx,
                ))
            })
            .collect()
    }

    fn look_menu(
        effect: &Effect,
        hues: openwoot_protocol::lighting::HueRange,
        cx: &Context<Self>,
    ) -> Div {
        let page = cx.entity().downgrade();
        let selected = HUE_RANGES.iter().position(|(range, _)| *range == hues);
        v_flex()
            .w_full()
            .gap_1()
            .child(param_label(format!("{} look", effect::name(effect)), cx))
            .child(
                Dropdown::new("effect-look")
                    .items(HUE_RANGES.iter().map(|(_, name)| Item::new(*name)))
                    .selected(selected)
                    .on_select(move |index, window, cx| {
                        let Some((range, _)) = HUE_RANGES.get(index) else {
                            return;
                        };
                        let range = *range;
                        page.update(cx, |page, cx| {
                            page.edit_focused(|effect| effect::set_hues(effect, range), window, cx);
                        })
                        .ok();
                    }),
            )
    }

    fn ripple_rows(ripple: KeyRipple, cx: &Context<Self>) -> [AnyElement; 2] {
        [
            switch_row(
                "Random colours",
                None,
                ripple.palette,
                |effect, enabled| {
                    if let Effect::KeyRipple(ripple) = effect {
                        ripple.palette = enabled;
                    }
                },
                cx,
            )
            .into_any_element(),
            switch_row(
                "Inverted",
                Some("Ripples darken the keys"),
                ripple.inverted,
                |effect, enabled| {
                    if let Effect::KeyRipple(ripple) = effect {
                        ripple.inverted = enabled;
                    }
                },
                cx,
            )
            .into_any_element(),
        ]
    }

    fn trail_rows(&self, fade: ReactiveFade, cx: &Context<Self>) -> Vec<AnyElement> {
        let paint = effect::paint_of(fade.coloring);
        let selected = TRAIL_PAINTS.iter().position(|(mode, _)| *mode == paint);
        let mode = v_flex()
            .w_full()
            .gap_1()
            .child(param_label("Colour mode", cx))
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(choices(
                        "trail-paint",
                        TRAIL_PAINTS.map(|(_, label)| label),
                        selected,
                        Choice::Filled,
                        cx.listener(|this, index: &usize, window, cx| {
                            if let Some((mode, _)) = TRAIL_PAINTS.get(*index) {
                                this.choose_trail_paint(*mode, window, cx);
                            }
                        }),
                        cx,
                    ))
                    .when(paint == Paint::Fixed, |this| {
                        this.child(
                            Swatch::new("trail-colour", &self.first_color).size(px(SWATCH_SIZE)),
                        )
                    }),
            );
        vec![
            mode.into_any_element(),
            choice_row(
                "Fade direction",
                "trail-direction",
                &FADE_DIRECTIONS,
                fade.inverted,
                |effect, inverted| {
                    if let Effect::ReactiveFade(fade) = effect {
                        fade.inverted = inverted;
                    }
                },
                cx,
            )
            .into_any_element(),
        ]
    }

    fn color_rows(&self, cx: &Context<Self>) -> [AnyElement; 2] {
        [
            ("Primary colour", "effect-primary", &self.first_color),
            ("Secondary colour", "effect-secondary", &self.second_color),
        ]
        .map(|(label, id, picker)| {
            h_flex()
                .gap_3()
                .items_center()
                .child(param_label(label, cx))
                .child(Swatch::new(id, picker).size(px(SWATCH_SIZE)))
                .into_any_element()
        })
    }

    fn choose_trail_paint(&mut self, mode: Paint, window: &mut Window, cx: &mut Context<Self>) {
        let fixed = self
            .first_color
            .read(cx)
            .value()
            .unwrap_or(super::WHITE)
            .opaque();
        let coloring = match mode {
            Paint::KeyColor => ReactiveColoring::KeyColor,
            Paint::Palette => ReactiveColoring::Palette,
            Paint::Fixed => ReactiveColoring::Fixed(fixed),
        };
        self.edit_focused(
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
}

fn choice_row<T: Copy + PartialEq + 'static, const N: usize>(
    label: &'static str,
    id: &'static str,
    options: &'static [(T, &'static str); N],
    current: T,
    change: fn(&mut Effect, T),
    cx: &Context<LightingPage>,
) -> Div {
    let selected = options.iter().position(|(value, _)| *value == current);
    v_flex()
        .w_full()
        .gap_1()
        .child(param_label(label, cx))
        .child(choices(
            id,
            options.iter().map(|(_, name)| *name),
            selected,
            Choice::Filled,
            cx.listener(move |this, index: &usize, window, cx| {
                if let Some((value, _)) = options.get(*index) {
                    let value = *value;
                    this.edit_focused(|effect| change(effect, value), window, cx);
                }
            }),
            cx,
        ))
}

fn switch_row(
    label: &'static str,
    help: Option<&'static str>,
    checked: bool,
    change: impl Fn(&mut Effect, bool) + 'static,
    cx: &Context<LightingPage>,
) -> Div {
    let id = SharedString::from(format!("effect-switch-{label}"));
    panel::toggle(
        label,
        help.map(SharedString::from),
        Switch::new(id).checked(checked).on_change(cx.listener(
            move |this, enabled: &bool, window, cx| {
                let enabled = *enabled;
                this.edit_focused(|effect| change(effect, enabled), window, cx);
            },
        )),
        cx,
    )
}
