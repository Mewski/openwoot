use std::{rc::Rc, time::Instant};

use gpui_kit::{
    App, Context, Div, DragMoveEvent, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    component::{
        ActiveTheme as _, h_flex,
        slider::{Slider, SliderState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px, relative,
};
use openwoot_protocol::{
    device::{Mode, PollingRate, Speed},
    gamepad::{Control, Normalization},
};

use super::{
    Carried, DeviceChange, GamepadPage, View,
    chart::{self, Chart},
    controls::{self, Controller},
    curve::{self, Curve, DEFAULT_UP_BIAS},
    glyph::{self, Ink},
};
use crate::{
    dropdown::{Dropdown, Item},
    keyboard::{Dragged, KeyFace, KeyboardView},
    page::Page,
    panel,
    store::Keyboard,
    switch::Switch,
    usage,
};

const VIEWS: [(View, &str); 3] = [
    (View::Setup, "Setup & Remap"),
    (View::Configuration, "Configuration"),
    (View::Tester, "Gamepad Tester"),
];
const CONTROLLERS: [Controller; 2] = [Controller::Xbox, Controller::Classic];
const POLLING_RATES: [(PollingRate, &str); 3] = [
    (PollingRate::Every125Microseconds, "8000 Hz"),
    (PollingRate::Every1Millisecond, "1000 Hz"),
    (PollingRate::Every4Milliseconds, "250 Hz"),
];
const ENABLE_WIDTH: f32 = 0.36;
const TAB_HEIGHT: f32 = 34.0;
const PRESET_TILE: f32 = 72.0;
const PRESET_CURVE: f32 = 44.0;
const VALUE_WIDTH: f32 = 44.0;
const DEGREES_PER_PERCENT: f32 = 0.9;
const DISABLED: f32 = 0.5;
const SAME_TRAVEL: f32 = 0.01;

impl Render for GamepadPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered_at = Instant::now();
        if !cx.has_active_drag() {
            self.carried_from = None;
        }
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return panel::page();
        };
        let board = self.keyboard_view(&keyboard, window, cx);
        let cards = match self.view {
            View::Setup => h_flex()
                .w_full()
                .items_start()
                .gap_4()
                .child(
                    v_flex()
                        .flex_none()
                        .w(relative(ENABLE_WIDTH))
                        .child(self.enable_card(cx)),
                )
                .child(v_flex().flex_1().min_w_0().child(self.mapping_card(cx))),
            View::Configuration => {
                panel::columns(self.curve_card(&keyboard, cx), self.response_card(cx))
            }
            View::Tester => self.tester_card(&keyboard, cx),
        };
        let body = v_flex()
            .w_full()
            .gap_5()
            .child(panel::heading(
                Page::Gamepad.title(),
                None,
                self.view_tabs(cx),
                cx,
            ))
            .child(cards);
        panel::page_with_stage("gamepad-body", panel::stage(board), body)
            .on_drop(cx.listener(|this, dragged: &Dragged<Carried>, _, cx| {
                this.drop_off_keyboard(*dragged.value(), cx);
            }))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<Dragged<Carried>>, _, cx| {
                    let carried = *event.drag(cx).value();
                    this.track_carried(carried, cx);
                }),
            )
    }
}

impl GamepadPage {
    fn view_tabs(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let (primary, border, chosen_surface, surface, hover, text, radius) = (
            theme.primary,
            theme.border,
            theme.secondary,
            theme.group_box,
            theme.secondary_hover,
            theme.foreground,
            theme.radius,
        );
        h_flex().gap_2().children(VIEWS.map(|(view, name)| {
            let chosen = self.view == view;
            div()
                .id(name)
                .h(px(TAB_HEIGHT))
                .px_3()
                .flex()
                .items_center()
                .rounded(radius)
                .border_1()
                .border_color(if chosen { primary } else { border })
                .bg(if chosen { chosen_surface } else { surface })
                .text_color(text)
                .when(!chosen, |this| {
                    this.cursor_pointer().hover(|style| style.bg(hover))
                })
                .child(name)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.choose_view(view, window, cx);
                }))
        }))
    }

    fn keyboard_view(
        &self,
        keyboard: &Keyboard,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> KeyboardView {
        let controller = self.shown_controller();
        let ink = Ink::of(cx);
        let typing_off = self.controller().is_some()
            && self
                .key_settings
                .is_some_and(|settings| !settings.keyboard_in_gamepad_mode);
        let testing = self.view == View::Tester;
        let view = KeyboardView::new("gamepad-keys", keyboard, |key| {
            self.key_face(keyboard, key, (controller, ink), typing_off, testing)
        })
        .fit_width(panel::content_width(window));
        let editable = self.view == View::Setup && self.controller().is_some();
        let Some(mapping) = self.mapping.as_ref().filter(|_| editable) else {
            return view;
        };
        let bindings: Vec<Option<Control>> = mapping.bindings.to_vec();
        let dropped = cx.entity().downgrade();
        let removed = dropped.clone();
        view.on_key_drop::<Carried>(move |key, carried, _, cx| {
            dropped
                .update(cx, |page, cx| page.drop_on_key(key, *carried, cx))
                .ok();
        })
        .on_key_drag::<Carried>(move |key| {
            let control = bindings.get(key).copied().flatten()?;
            Some(
                Dragged::new(
                    Carried::Binding { key, control },
                    controls::name(control, controller),
                )
                .glyph(glyph::glyph(control, controller, ink, false)),
            )
        })
        .on_key_secondary_click(move |key, _, cx| {
            removed
                .update(cx, |page, cx| page.remove_binding(key, cx))
                .ok();
        })
    }

    fn key_face(
        &self,
        keyboard: &Keyboard,
        key: usize,
        (controller, ink): (Controller, Ink),
        typing_off: bool,
        testing: bool,
    ) -> KeyFace {
        if self.carried_from == Some(key) {
            return KeyFace::default();
        }
        match self.binding(key) {
            Some(control) => KeyFace {
                glyph: Some(glyph::glyph(
                    control,
                    controller,
                    ink,
                    !controller.sends(control),
                )),
                tooltip: Some(controls::tooltip(control, controller).into()),
                travel: testing
                    .then(|| self.key_travel(key).map(curve::travel_level))
                    .flatten(),
                ..KeyFace::default()
            },
            None => KeyFace {
                legend: key_legend(keyboard, key),
                dimmed: typing_off || testing,
                ..KeyFace::default()
            },
        }
    }

    fn enable_card(&self, cx: &mut Context<Self>) -> Div {
        let enabled = self.controller().is_some();
        panel::section_with(
            "Enable gamepad",
            Some("The keyboard works like a controller and sends analog keys.".into()),
            Switch::new("gamepad-enabled")
                .checked(enabled)
                .disabled(self.device.is_none())
                .on_change(cx.listener(|this, checked: &bool, window, cx| {
                    let mode = if *checked {
                        Controller::PREFERRED.mode()
                    } else {
                        Mode::NoGamepad
                    };
                    this.confirm_device_change(DeviceChange::Mode(mode), window, cx);
                })),
            cx,
        )
        .child(panel::field("Controller type", self.type_dropdown(cx), cx))
        .child(panel::field("Polling rate", self.polling_dropdown(cx), cx))
        .child(panel::note(
            "These settings affect every profile that uses the gamepad.",
            cx,
        ))
    }

    fn type_dropdown(&self, cx: &mut Context<Self>) -> Dropdown {
        let current = self.controller();
        let shown = self.shown_controller();
        let page = cx.entity().downgrade();
        Dropdown::new("gamepad-type")
            .items(CONTROLLERS.map(|controller| {
                let item = Item::new(controller.name());
                match controller {
                    Controller::Classic => item.detail("HID gamepad"),
                    Controller::Xbox if cfg!(target_os = "macos") => item.detail("Not on macOS"),
                    Controller::Xbox => item,
                }
            }))
            .selected(
                CONTROLLERS
                    .iter()
                    .position(|controller| *controller == shown),
            )
            .disabled(current.is_none())
            .on_select(move |index, window, cx| {
                let Some(chosen) = CONTROLLERS.get(index).copied() else {
                    return;
                };
                if Some(chosen) == current {
                    return;
                }
                page.update(cx, |page, cx| {
                    page.confirm_device_change(DeviceChange::Mode(chosen.mode()), window, cx);
                })
                .ok();
            })
    }

    fn polling_dropdown(&self, cx: &mut Context<Self>) -> Dropdown {
        let device = self.device;
        let current = device.map(|settings| settings.gamepad_polling);
        let full_speed = device.is_some_and(|settings| settings.speed == Speed::Full);
        let page = cx.entity().downgrade();
        Dropdown::new("gamepad-polling")
            .items(POLLING_RATES.map(|(rate, name)| {
                let item = Item::new(name);
                match rate {
                    PollingRate::Every125Microseconds if full_speed => {
                        item.detail("Above keyboard rate").disabled(true)
                    }
                    PollingRate::Every4Milliseconds => item.detail("Compatibility"),
                    _ => item,
                }
            }))
            .selected(
                POLLING_RATES
                    .iter()
                    .position(|(rate, _)| Some(*rate) == current),
            )
            .disabled(self.controller().is_none())
            .on_select(move |index, window, cx| {
                let Some((rate, _)) = POLLING_RATES.get(index).copied() else {
                    return;
                };
                if Some(rate) == current {
                    return;
                }
                page.update(cx, |page, cx| {
                    page.confirm_device_change(DeviceChange::Polling(rate), window, cx);
                })
                .ok();
            })
    }

    fn mapping_card(&self, cx: &mut Context<Self>) -> Div {
        let enabled = self.controller().is_some() && self.mapping.is_some();
        let exclusive = self.mapping.as_ref().map(|mapping| mapping.exclusive);
        let keyboard_keys = self
            .key_settings
            .map(|settings| settings.keyboard_in_gamepad_mode);
        panel::section(
            "Controller mapping",
            Some("Drag buttons onto keys; right-click a key to remove its button.".into()),
            cx,
        )
        .child(self.palette(enabled, cx))
        .child(panel::toggle(
            "Enable keyboard keys",
            Some("Keys keep typing while the gamepad is on.".into()),
            Switch::new("keyboard-in-gamepad-mode")
                .checked(keyboard_keys.unwrap_or(true))
                .disabled(!enabled || keyboard_keys.is_none())
                .on_change(cx.listener(|this, checked: &bool, window, cx| {
                    this.set_keyboard_keys(*checked, window, cx);
                })),
            cx,
        ))
        .child(panel::toggle(
            "Gamepad mapping override",
            Some("Keys with a gamepad button send no keyboard input.".into()),
            Switch::new("gamepad-exclusive")
                .checked(exclusive.unwrap_or(false))
                .disabled(!enabled || exclusive.is_none() || keyboard_keys == Some(false))
                .on_change(cx.listener(|this, checked: &bool, _, cx| {
                    this.set_exclusive(*checked, cx);
                })),
            cx,
        ))
    }

    fn curve_card(&self, keyboard: &Keyboard, cx: &mut Context<Self>) -> Div {
        let card = panel::section(
            "Analog curve",
            Some("How deep a key is pressed becomes the analog value.".into()),
            cx,
        );
        let Some(settings) = self.analog else {
            return card;
        };
        let enabled = self.controller().is_some();
        let handler: Option<chart::Handler> = enabled.then(|| {
            let page = cx.entity().downgrade();
            let handler: chart::Handler = Rc::new(move |input, window, cx| {
                page.update(cx, |page, cx| page.on_chart(input, window, cx))
                    .ok();
            });
            handler
        });
        let markers = (0..keyboard.key_count())
            .filter(|key| self.binding(*key).is_some_and(controls::is_analog))
            .filter_map(|key| self.key_travel(key))
            .filter(|travel| *travel > 0)
            .map(|travel| {
                let level = curve::travel_level(travel);
                (level, curve::output(settings.curve, level))
            })
            .collect();
        card.child(chart::chart(
            Chart {
                curve: settings.curve,
                travel_mm: board_travel(keyboard),
                focused: self.dragged_point.or(self.hovered_point),
                markers,
                handler,
            },
            cx,
        ))
        .child(panel::field(
            "Select a preset",
            Self::presets(settings.curve, enabled, cx),
            cx,
        ))
    }

    fn presets(current: Curve, enabled: bool, cx: &mut Context<Self>) -> Div {
        let chosen = curve::preset_of(current);
        let theme = cx.theme();
        let (primary, border, surface, hover, text, radius) = (
            theme.primary,
            theme.border,
            theme.secondary,
            theme.secondary_hover,
            theme.foreground,
            theme.radius,
        );
        h_flex()
            .w_full()
            .flex_wrap()
            .gap_2()
            .children(curve::PRESETS.iter().enumerate().map(|(index, preset)| {
                let points = preset.curve;
                v_flex()
                    .id(("curve-preset", index))
                    .w(px(PRESET_TILE))
                    .h(px(PRESET_TILE))
                    .p_1()
                    .items_center()
                    .rounded(radius)
                    .bg(surface)
                    .border_1()
                    .border_color(if chosen == Some(index) {
                        primary
                    } else {
                        border
                    })
                    .child(
                        div()
                            .w_full()
                            .h(px(PRESET_CURVE))
                            .child(chart::thumbnail(points, text)),
                    )
                    .child(div().text_xs().text_color(text).child(preset.name))
                    .when(enabled, |this| {
                        this.cursor_pointer()
                            .hover(|style| style.bg(hover))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.change_analog(|settings| settings.curve = points, window, cx);
                            }))
                    })
                    .when(!enabled, |this| this.opacity(DISABLED))
            }))
    }

    fn response_card(&self, cx: &mut Context<Self>) -> Div {
        let settings = self.analog;
        let enabled = self.controller().is_some() && settings.is_some();
        let bias = settings.and_then(|settings| settings.left_stick_up_bias);
        panel::section("Gamepad response", None, cx)
            .child(panel::toggle(
                "Enable angle adjustment",
                Some("Moves the centre of the forward diagonal of the left stick.".into()),
                Switch::new("joystick-angle")
                    .checked(bias.is_some())
                    .disabled(!enabled)
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        let bias = checked.then_some(DEFAULT_UP_BIAS);
                        this.change_analog(
                            |settings| settings.left_stick_up_bias = bias,
                            window,
                            cx,
                        );
                    })),
                cx,
            ))
            .when_some(bias.filter(|_| enabled), |this, bias| {
                this.child(panel::field(
                    "Adjust the diagonal angle",
                    slider_with_value(&self.up_bias, degrees(bias), cx),
                    cx,
                ))
            })
            .child(panel::toggle(
                "Use square joystick output",
                Some("Full output on both axes at once; some games disagree.".into()),
                Switch::new("square-joystick")
                    .checked(
                        settings.is_some_and(|settings| {
                            settings.normalization == Normalization::Square
                        }),
                    )
                    .disabled(!enabled)
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        let normalization = if *checked {
                            Normalization::Square
                        } else {
                            Normalization::Circular
                        };
                        this.change_analog(
                            |settings| settings.normalization = normalization,
                            window,
                            cx,
                        );
                    })),
                cx,
            ))
            .child(panel::toggle(
                "Snappy joystick",
                Some("Of two opposite keys, the deeper one wins.".into()),
                Switch::new("snappy-joystick")
                    .checked(settings.is_some_and(|settings| settings.single_direction_axes))
                    .disabled(!enabled)
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        let snappy = *checked;
                        this.change_analog(
                            |settings| settings.single_direction_axes = snappy,
                            window,
                            cx,
                        );
                    })),
                cx,
            ))
    }
}

fn slider_with_value(slider: &Entity<SliderState>, value: String, cx: &App) -> Div {
    h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .child(div().flex_1().child(Slider::new(slider)))
        .child(
            div()
                .w(px(VALUE_WIDTH))
                .text_right()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(value),
        )
}

fn degrees(percent: u8) -> String {
    format!("{:.0}°", f32::from(percent) * DEGREES_PER_PERCENT)
}

fn board_travel(keyboard: &Keyboard) -> f32 {
    let mut seen: Vec<(f32, usize)> = Vec::new();
    for key in (0..keyboard.key_count()).filter(|key| keyboard.has_key(*key)) {
        let travel = keyboard.travel_mm(key);
        match seen
            .iter_mut()
            .find(|(known, _)| (*known - travel).abs() < SAME_TRAVEL)
        {
            Some((_, count)) => *count += 1,
            None => seen.push((travel, 1)),
        }
    }
    seen.into_iter()
        .max_by_key(|(_, count)| *count)
        .map_or(4.0, |(travel, _)| travel)
}

fn key_legend(keyboard: &Keyboard, key: usize) -> SharedString {
    keyboard
        .legends
        .get(key)
        .map_or_else(SharedString::default, |action| {
            SharedString::from(usage::legend(*action).into_owned())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_diagonal_angle_reads_in_degrees() {
        assert_eq!(degrees(50), "45°");
        assert_eq!(degrees(100), "90°");
        assert_eq!(degrees(0), "0°");
    }
}
