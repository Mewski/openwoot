use std::{rc::Rc, time::Instant};

use gpui_kit::{
    App, Context, Div, DragMoveEvent, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    component::{ActiveTheme as _, h_flex, input::Input, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    device::{Mode, PollingRate, SettingsState, Speed},
    gamepad::{Control, Normalization},
};

use super::{
    Carried, DeviceChange, GamepadPage, View,
    chart::{self, Chart},
    controls::{self, Controller},
    curve,
    glyph::{self, Ink},
    system,
};
use crate::{
    dropdown::{Dropdown, Item},
    keyboard::{Dragged, KeyFace, KeyboardView},
    page::Page,
    panel::{self, ActionButton, Stage},
    slider::Slider,
    store::Keyboard,
    switch::Switch,
    theme,
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
const ENABLE_CARD_BASIS: f32 = 662.0;
const CURVE_CARD_BASIS: f32 = 572.0;
const CARD_MIN_HEIGHT: f32 = 360.0;
const CARDS_GAP: f32 = 16.0;
const SETTINGS_GAP: f32 = 12.0;
const FIELDS_GAP: f32 = 20.0;
const CURVE_GAP: f32 = 24.0;
const RESPONSE_GAP: f32 = 32.0;
const ANGLE_GAP: f32 = 20.0;
const ANGLE_BLOCK_HEIGHT: f32 = 56.0;
const ANGLE_ITEM_GAP: f32 = 8.0;
const ANGLE_LABEL_TEXT: f32 = 12.8;
const ANGLE_LABEL_LINE: f32 = 15.36;
const ANGLE_ROW_TOP: f32 = 12.0;
const ANGLE_FIELD_WIDTH: f32 = 48.0;
const ANGLE_FIELD_PADDING: f32 = 8.0;
const ANGLE_FIELD_TEXT: f32 = 12.0;
const DEGREE_TEXT: f32 = 12.8;
const TAB_HEIGHT: f32 = 34.0;
const TAB_GAP: f32 = 6.0;
const TAB_PADDING: f32 = 12.0;
const TAB_RADIUS: f32 = 7.0;
const TAB_TEXT: f32 = 16.0;
const TAB_LINE: f32 = 24.0;
const PRESET_TILE: f32 = 68.0;
const PRESET_GAP: f32 = 8.0;
const PRESET_INNER_GAP: f32 = 4.0;
const PRESET_LABEL_ROW: f32 = 32.0;
const PRESET_LABEL_GAP: f32 = 8.0;
const PRESET_PICTURE: f32 = 46.4;
const PRESET_RADIUS: f32 = 6.0;
const PRESET_LABEL_TEXT: f32 = 12.0;
const PRESET_LABEL_LINE: f32 = 14.4;
const ANGLE_FIELD_RADIUS: f32 = 6.0;
const DISABLED: f32 = 0.4;
const SAME_TRAVEL: f32 = 0.01;

impl Render for GamepadPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered_at = Instant::now();
        if !cx.has_active_drag() {
            self.carried_from = None;
        }
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return panel::page();
        };
        let board = self.keyboard_view(&keyboard, cx);
        let cards = match self.view {
            View::Setup => card_pair(
                self.enable_card(cx),
                ENABLE_CARD_BASIS,
                self.mapping_card(cx),
            ),
            View::Configuration => card_pair(
                self.curve_card(&keyboard, cx),
                CURVE_CARD_BASIS,
                self.response_card(cx),
            ),
            View::Tester => self.tester_card(&keyboard, cx).min_h(px(CARD_MIN_HEIGHT)),
        };
        panel::page_with_stage(
            "gamepad-body",
            Stage::new(board),
            panel::heading(Page::Gamepad.title()).centre(self.view_tabs(cx)),
            cards,
        )
        .on_drop(cx.listener(|this, dragged: &Dragged<Carried>, _, cx| {
            this.drop_off_keyboard(*dragged.value(), cx);
        }))
        .on_drag_move(cx.listener(
            |this, event: &DragMoveEvent<Dragged<Carried>>, _, cx| {
                let carried = *event.drag(cx).value();
                this.track_carried(carried, cx);
            },
        ))
    }
}

impl GamepadPage {
    fn view_tabs(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let (idle, idle_border, idle_text) =
            (theme.accent, theme.border, theme.group_box_foreground);
        let (raised, raised_border, chosen_border, chosen_text) = (
            theme.secondary,
            theme.secondary_hover,
            theme.primary,
            theme.foreground,
        );
        h_flex()
            .gap(px(TAB_GAP))
            .children(VIEWS.map(|(view, name)| {
                let chosen = self.view == view;
                div()
                    .id(name)
                    .h(px(TAB_HEIGHT))
                    .px(px(TAB_PADDING))
                    .flex()
                    .items_center()
                    .rounded(px(TAB_RADIUS))
                    .border_1()
                    .text_size(px(TAB_TEXT))
                    .line_height(px(TAB_LINE))
                    .map(|this| {
                        if chosen {
                            this.bg(raised)
                                .border_color(chosen_border)
                                .text_color(chosen_text)
                        } else {
                            this.bg(idle)
                                .border_color(idle_border)
                                .text_color(idle_text)
                                .cursor_pointer()
                                .hover(|style| style.bg(raised).border_color(raised_border))
                                .active(|style| {
                                    style
                                        .bg(raised)
                                        .border_color(chosen_border)
                                        .text_color(chosen_text)
                                })
                        }
                    })
                    .child(name)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose_view(view, window, cx);
                    }))
            }))
    }

    fn keyboard_view(&self, keyboard: &Keyboard, cx: &mut Context<Self>) -> KeyboardView {
        let controller = self.shown_controller();
        let ink = Ink::of(cx);
        let typing_off = self.controller().is_some()
            && self
                .key_settings
                .is_some_and(|settings| !settings.keyboard_in_gamepad_mode);
        let testing = self.view == View::Tester;
        let view = KeyboardView::new("gamepad-keys", keyboard, |key| {
            self.key_face(keyboard, key, (controller, ink), typing_off, testing)
        });
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
        let notice = if self.restart_pending() {
            Self::restart_notice(cx)
        } else {
            panel::info("These settings affect every profile that uses gamepad.", cx)
        };
        panel::section_with(
            "Enable Gamepad",
            Some(
                "Your Wooting keyboard can work just like a controller, sending out analog key \
                 signals."
                    .into(),
            ),
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
        .child(
            v_flex()
                .w_full()
                .gap(px(SETTINGS_GAP))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(FIELDS_GAP))
                        .child(panel::field("Controller Type", self.type_dropdown(cx), cx))
                        .child(panel::field_with_help(
                            "Polling Rate",
                            "Gamepad polling rate controls how quickly your keyboard sends \
                             gamepad inputs to your computer.",
                            self.polling_dropdown(cx),
                            cx,
                        )),
                )
                .child(notice),
        )
    }

    fn restart_notice(cx: &mut Context<Self>) -> Div {
        panel::info("Restart your device to apply.", cx)
            .items_center()
            .child(
                ActionButton::new("gamepad-restart", "Restart")
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| this.restart(window, cx))),
            )
    }

    fn type_dropdown(&self, cx: &mut Context<Self>) -> Dropdown {
        let current = self.controller();
        let shown = self.shown_controller();
        let page = cx.entity().downgrade();
        Dropdown::new("gamepad-type")
            .items(CONTROLLERS.map(|controller| {
                let item = Item::new(controller.name());
                match (controller, system::WITHOUT_XBOX_DRIVER) {
                    (Controller::Classic, _) => item.detail("HID Gamepad"),
                    (Controller::Xbox, Some(system)) => item.detail(format!("Not on {system}")),
                    (Controller::Xbox, None) => item,
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
        let state = self.device;
        let current = state.map(|state| state.chosen.gamepad_polling);
        let limit = state.and_then(fastest_polling_limit);
        let page = cx.entity().downgrade();
        Dropdown::new("gamepad-polling")
            .items(POLLING_RATES.map(|(rate, name)| {
                let item = Item::new(name);
                match (rate, limit) {
                    (PollingRate::Every125Microseconds, Some(limit)) => {
                        item.detail(limit).disabled(true)
                    }
                    (PollingRate::Every4Milliseconds, _) => item.detail("Compatibility"),
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
            "Controller Mapping",
            Some("Add gamepad buttons by drag and drop, right click a key to remove one.".into()),
            cx,
        )
        .child(self.palette(enabled, cx))
        .child(panel::toggle_described(
            "Enable keyboard keys",
            "Enable digital keys if you want to output keyboard keys for typing.",
            Switch::new("keyboard-in-gamepad-mode")
                .checked(keyboard_keys.unwrap_or(true))
                .disabled(!enabled || keyboard_keys.is_none())
                .on_change(cx.listener(|this, checked: &bool, window, cx| {
                    this.set_keyboard_keys(*checked, window, cx);
                })),
            cx,
        ))
        .child(panel::toggle_described(
            "Gamepad mapping override",
            "Disable the keyboard input on keys with gamepad bindings on them.",
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
        let card = panel::section_with_help(
            "Analog Curve",
            "This modifies how your keypress translates to the analog signal of a controller.",
            None,
            None,
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
        card.child(
            v_flex()
                .w_full()
                .gap(px(CURVE_GAP))
                .child(chart::chart(
                    Chart {
                        curve: settings.curve,
                        travel_mm: board_travel(keyboard),
                        focused: self.dragged_point.or(self.hovered_point),
                        markers,
                        handler,
                    },
                    cx,
                ))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(PRESET_LABEL_GAP))
                        .child(
                            h_flex()
                                .h(px(PRESET_LABEL_ROW))
                                .child(panel::label("Select a preset", cx)),
                        )
                        .child(Self::presets(enabled, cx)),
                ),
        )
    }

    fn presets(enabled: bool, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let (surface, border, hover, pressed) = (
            theme.secondary,
            theme.border,
            theme.secondary_hover,
            theme.secondary_active,
        );
        let (surface, text) = if enabled {
            (surface, theme.foreground)
        } else {
            (theme.accent, theme.muted_foreground)
        };
        h_flex().w_full().flex_wrap().gap(px(PRESET_GAP)).children(
            curve::PRESETS.iter().enumerate().map(|(index, preset)| {
                let points = preset.curve;
                v_flex()
                    .id(("curve-preset", index))
                    .flex_none()
                    .size(px(PRESET_TILE))
                    .justify_center()
                    .items_center()
                    .gap(px(PRESET_INNER_GAP))
                    .rounded(px(PRESET_RADIUS))
                    .border_1()
                    .border_color(border)
                    .bg(surface)
                    .child(
                        div()
                            .w_full()
                            .h(px(PRESET_PICTURE))
                            .child(chart::thumbnail(points, text)),
                    )
                    .child(
                        div()
                            .text_size(px(PRESET_LABEL_TEXT))
                            .line_height(px(PRESET_LABEL_LINE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text)
                            .child(preset.name),
                    )
                    .map(|this| {
                        if enabled {
                            this.cursor_pointer()
                                .hover(|style| style.bg(hover).border_color(hover))
                                .active(|style| style.bg(pressed).border_color(hover))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.change_analog(
                                        |settings| settings.curve = points,
                                        window,
                                        cx,
                                    );
                                }))
                        } else {
                            this.cursor_not_allowed().opacity(DISABLED)
                        }
                    })
            }),
        )
    }

    fn response_card(&self, cx: &mut Context<Self>) -> Div {
        let settings = self.analog;
        let enabled = self.controller().is_some() && settings.is_some();
        let angled = settings.is_some_and(|settings| settings.left_stick_up_bias.enabled);
        panel::section("Gamepad Response", None, cx)
            .gap(px(RESPONSE_GAP))
            .child(
                v_flex()
                    .w_full()
                    .gap(px(ANGLE_GAP))
                    .child(panel::toggle_described(
                        "Enable angle adjustment",
                        "Modify the center point of the diagonal direction when pressing \
                         forward and left/right.",
                        Switch::new("joystick-angle")
                            .checked(angled)
                            .disabled(!enabled)
                            .on_change(cx.listener(|this, checked: &bool, window, cx| {
                                let angled = *checked;
                                this.change_analog(
                                    |settings| settings.left_stick_up_bias.enabled = angled,
                                    window,
                                    cx,
                                );
                            })),
                        cx,
                    ))
                    .when(angled && enabled, |this| this.child(self.angle_block(cx))),
            )
            .child(panel::toggle_described(
                "Use Square joystick output",
                "Restrict the joystick movement vector to a square instead of a circle.",
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
            .child(panel::toggle_described(
                "Snappy Joystick",
                "Use the maximum value of opposite sides of the axis rather than combining them.",
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

    fn angle_block(&self, cx: &App) -> Div {
        let palette = theme::palette();
        let theme = cx.theme();
        v_flex()
            .w_full()
            .h(px(ANGLE_BLOCK_HEIGHT))
            .items_center()
            .gap(px(ANGLE_ITEM_GAP))
            .child(
                div()
                    .flex_none()
                    .text_size(px(ANGLE_LABEL_TEXT))
                    .line_height(px(ANGLE_LABEL_LINE))
                    .font_weight(FontWeight::BOLD)
                    .text_color(palette.heading)
                    .child("Adjust the diagonal angle"),
            )
            .child(
                h_flex()
                    .flex_none()
                    .w_full()
                    .pt(px(ANGLE_ROW_TOP))
                    .gap(px(ANGLE_ITEM_GAP))
                    .child(div().flex_1().min_w_0().child(Slider::new(&self.up_bias)))
                    .child(
                        Input::new(&self.angle)
                            .w(px(ANGLE_FIELD_WIDTH))
                            .px(px(ANGLE_FIELD_PADDING))
                            .rounded(px(ANGLE_FIELD_RADIUS))
                            .bg(palette.input)
                            .text_size(px(ANGLE_FIELD_TEXT))
                            .text_center(),
                    )
                    .child(
                        div()
                            .text_size(px(DEGREE_TEXT))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child("°"),
                    ),
            )
    }
}

fn card_pair(fitted: Div, fitted_basis: f32, filling: Div) -> Div {
    h_flex()
        .w_full()
        .items_start()
        .gap(px(CARDS_GAP))
        .child(
            fitted
                .flex_basis(px(fitted_basis))
                .min_w_0()
                .min_h(px(CARD_MIN_HEIGHT)),
        )
        .child(filling.w_full().min_w_0().min_h(px(CARD_MIN_HEIGHT)))
}

fn fastest_polling_limit(state: SettingsState) -> Option<&'static str> {
    if state.chosen.speed == Speed::Full {
        Some("Above keyboard rate")
    } else if state.negotiated == Some(Speed::Full) {
        Some("Not on full-speed USB")
    } else {
        None
    }
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
        .map_or_else(SharedString::default, |action| keyboard.legend(*action))
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::device::Settings;

    use super::*;

    fn state(chosen: Speed, negotiated: Option<Speed>) -> SettingsState {
        let settings = Settings {
            speed: chosen,
            ..Settings::default()
        };
        SettingsState {
            active: settings,
            chosen: settings,
            negotiated,
        }
    }

    #[test]
    fn eight_kilohertz_needs_a_high_speed_keyboard_and_link() {
        assert_eq!(
            fastest_polling_limit(state(Speed::High, Some(Speed::High))),
            None
        );
        assert_eq!(fastest_polling_limit(state(Speed::High, None)), None);
        assert_eq!(
            fastest_polling_limit(state(Speed::High, Some(Speed::Full))),
            Some("Not on full-speed USB")
        );
        assert_eq!(
            fastest_polling_limit(state(Speed::Full, Some(Speed::Full))),
            Some("Above keyboard rate")
        );
    }
}
