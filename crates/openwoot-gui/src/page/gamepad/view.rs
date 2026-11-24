use std::rc::Rc;

use gpui_kit::{
    App, Context, Div, Entity, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    component::{
        ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _,
        button::{Button, ButtonGroup, ButtonVariants},
        h_flex,
        slider::{Slider, SliderState},
        switch::Switch,
        tooltip::Tooltip,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    device::{Mode, PollingRate, Speed},
    gamepad::{CURVE_POINTS, Control, Normalization},
};

use super::{
    GamepadPage, View,
    chart::{self, Chart},
    controls::{self, Support},
    curve::{self, DEFAULT_UP_BIAS, POINT_SCALE},
};
use crate::{
    keyboard::{KeyFace, KeyboardView},
    page::Page,
    panel,
    store::Keyboard,
    usage,
};

const VALUE_WIDTH: f32 = 44.0;
const GROUP_LABEL_WIDTH: f32 = 72.0;
const VIEWS: [(View, &str); 3] = [
    (View::Mapping, "Setup & Remap"),
    (View::Analog, "Configuration"),
    (View::Test, "Gamepad Tester"),
];
const CONTROLLER_TYPES: [(Mode, &str); 2] = [(Mode::Xinput, "Xbox"), (Mode::Gamepad, "Classic")];
const POLLING_RATES: [(PollingRate, &str); 3] = [
    (PollingRate::Every4Milliseconds, "250 Hz"),
    (PollingRate::Every1Millisecond, "1000 Hz"),
    (PollingRate::Every125Microseconds, "8000 Hz"),
];
const POINT_NAMES: [&str; CURVE_POINTS] = ["Dead zone", "Knee 1", "Knee 2", "Saturation"];

impl Render for GamepadPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered_at = std::time::Instant::now();
        let page = panel::page();
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return page;
        };
        let board = match self.view {
            View::Mapping | View::Analog => self.mapping_keyboard(&keyboard, window, cx),
            View::Test => self.test_keyboard(&keyboard, window),
        };
        let page = page
            .child(panel::stage(board))
            .when(self.view == View::Mapping, |this| {
                this.child(self.caption(&keyboard, cx))
            })
            .child(panel::heading(
                Page::Gamepad.title(),
                Some("Use the keyboard as an analog controller.".into()),
                self.view_tabs(cx),
                cx,
            ));
        match self.view {
            View::Mapping => {
                page.child(panel::columns(self.enable_card(cx), self.mapping_card(cx)))
            }
            View::Analog => page.child(panel::columns(self.curve_card(cx), self.joystick_card(cx))),
            View::Test => page.child(panel::columns(
                self.live_curve_card(&keyboard, cx),
                self.identify_card(cx),
            )),
        }
    }
}

impl GamepadPage {
    fn view_tabs(&self, cx: &mut Context<Self>) -> Div {
        h_flex().gap_2().children(VIEWS.map(|(view, name)| {
            Button::new(name)
                .outline()
                .label(name)
                .chosen(self.view == view)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.choose_view(view, window, cx);
                }))
        }))
    }

    fn caption(&self, keyboard: &Keyboard, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let (muted, primary, danger) = (theme.muted_foreground, theme.primary, theme.danger);
        let mode = self.configured_mode();
        let unsupported = (0..keyboard.key_count())
            .filter(|key| keyboard.has_key(*key))
            .filter_map(|key| self.binding(key))
            .any(|control| mode != Mode::NoGamepad && !controls::works_in(control, mode));
        let line = |text: String, color: Hsla| {
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(SharedString::from(text.to_uppercase()))
        };
        let status = match self.selection.len() {
            0 => line("Select keys to map gamepad controls".to_owned(), muted),
            1 => line("1 key selected".to_owned(), primary),
            count => line(format!("{count} keys selected"), primary),
        };
        v_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(div().id("gamepad-selection-count").child(status).tooltip(
                        |window, cx| {
                            Tooltip::new("Shift- or Cmd-click to add keys").build(window, cx)
                        },
                    ))
                    .child(self.selection_actions(cx)),
            )
            .when(unsupported, |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().size_2().rounded_full().bg(danger))
                        .child(line(
                            "Red keys are not sent by this controller type".to_owned(),
                            muted,
                        )),
                )
            })
    }

    fn mapping_keyboard(
        &self,
        keyboard: &Keyboard,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> KeyboardView {
        let mode = self.configured_mode();
        let theme = cx.theme();
        let (mapped, unsupported) = (theme.info, theme.danger);
        let page = cx.entity().downgrade();
        KeyboardView::new("gamepad-mapping", keyboard, |key| {
            mapping_face(keyboard, key, self.binding(key), mode, mapped, unsupported)
        })
        .selection(&self.selection)
        .fit_width(panel::content_width(window))
        .on_key_click(move |key, event, _, cx| {
            page.update(cx, |page, cx| {
                page.selection.click(key, event.modifiers());
                cx.notify();
            })
            .ok();
        })
    }

    fn test_keyboard(&self, keyboard: &Keyboard, window: &Window) -> KeyboardView {
        let curve = self.analog.map(|settings| settings.curve);
        KeyboardView::new("gamepad-test", keyboard, |key| {
            test_face(
                keyboard,
                key,
                self.binding(key),
                self.key_travel(key),
                curve,
            )
        })
        .fit_width(panel::content_width(window))
    }

    fn enable_card(&self, cx: &mut Context<Self>) -> Div {
        let device = self.device;
        let mode = device.map(|settings| settings.mode);
        let enabled = mode.is_some_and(|mode| mode != Mode::NoGamepad);
        let pending = matches!(
            (device, self.started_with),
            (Some(now), Some(started))
                if (now.mode, now.gamepad_polling) != (started.mode, started.gamepad_polling)
        );
        panel::section_with(
            "Enable gamepad",
            Some("The keyboard can send analog keys as a controller.".into()),
            Switch::new("gamepad-enabled")
                .checked(enabled)
                .disabled(device.is_none())
                .on_change(cx.listener(|this, checked: &bool, window, cx| {
                    let mode = if !*checked {
                        Mode::NoGamepad
                    } else if cfg!(target_os = "macos") {
                        Mode::Gamepad
                    } else {
                        Mode::Xinput
                    };
                    this.change_device(move |settings| settings.mode = mode, window, cx);
                })),
            cx,
        )
        .when(enabled, |this| {
            this.child(panel::field(
                "Controller type",
                segmented(
                    ButtonGroup::new("gamepad-type")
                        .children(CONTROLLER_TYPES.map(|(entry, name)| {
                            Button::new(name)
                                .flex_1()
                                .label(name)
                                .chosen(mode == Some(entry))
                        }))
                        .on_click(cx.listener(|this, clicks: &Vec<usize>, window, cx| {
                            if let Some((mode, _)) = clicks
                                .first()
                                .and_then(|index| CONTROLLER_TYPES.get(*index))
                            {
                                let mode = *mode;
                                this.change_device(
                                    move |settings| settings.mode = mode,
                                    window,
                                    cx,
                                );
                            }
                        })),
                ),
                cx,
            ))
            .child(self.polling_field(cx))
        })
        .when(
            cfg!(target_os = "macos") && mode == Some(Mode::Xinput),
            |this| this.child(panel::note("macOS has no Xbox controller support.", cx)),
        )
        .child(panel::note(
            "These settings affect every profile and apply after a restart.",
            cx,
        ))
        .when(pending, |this| {
            this.child(
                Button::new("gamepad-restart")
                    .primary()
                    .w_full()
                    .label("Save and restart")
                    .on_click(cx.listener(|this, _, window, cx| this.save_and_restart(window, cx))),
            )
        })
    }

    fn polling_field(&self, cx: &mut Context<Self>) -> Div {
        let polling = self.device.map(|settings| settings.gamepad_polling);
        let full_speed = self
            .device
            .is_some_and(|settings| settings.speed == Speed::Full);
        let rates: Vec<(PollingRate, &str)> = POLLING_RATES
            .into_iter()
            .filter(|(rate, _)| !(full_speed && *rate == PollingRate::Every125Microseconds))
            .collect();
        panel::field(
            "Polling rate",
            segmented(
                ButtonGroup::new("gamepad-polling")
                    .children(rates.iter().map(|(rate, name)| {
                        Button::new(*name)
                            .flex_1()
                            .label(*name)
                            .chosen(polling == Some(*rate))
                    }))
                    .on_click(cx.listener(move |this, clicks: &Vec<usize>, window, cx| {
                        if let Some((rate, _)) = clicks.first().and_then(|index| rates.get(*index))
                        {
                            let rate = *rate;
                            this.change_device(
                                move |settings| settings.gamepad_polling = rate,
                                window,
                                cx,
                            );
                        }
                    })),
            ),
            cx,
        )
    }

    fn mapping_card(&self, cx: &mut Context<Self>) -> Div {
        let exclusive = self.mapping.as_ref().map(|mapping| mapping.exclusive);
        let keyboard_keys = self
            .key_settings
            .map(|settings| settings.keyboard_in_gamepad_mode);
        panel::section(
            "Controller mapping",
            Some("Select keys, then pick the control they send.".into()),
            cx,
        )
        .child(self.control_groups(cx))
        .child(panel::row(
            "Enable keyboard keys",
            Some("Keys keep typing while the gamepad is on.".into()),
            Switch::new("keyboard-in-gamepad-mode")
                .checked(keyboard_keys.unwrap_or(true))
                .disabled(keyboard_keys.is_none())
                .on_change(cx.listener(|this, checked: &bool, window, cx| {
                    this.set_keyboard_keys(*checked, window, cx);
                })),
            cx,
        ))
        .child(panel::row(
            "Gamepad mapping override",
            Some("Mapped keys send no keyboard input.".into()),
            Switch::new("gamepad-exclusive")
                .checked(exclusive.unwrap_or(false))
                .disabled(exclusive.is_none())
                .on_change(cx.listener(|this, checked: &bool, _, cx| {
                    let exclusive = *checked;
                    this.change_mapping(|mapping, _| mapping.exclusive = exclusive, cx);
                })),
            cx,
        ))
    }

    fn selection_actions(&self, cx: &mut Context<Self>) -> Div {
        let selected = self.selection.len();
        let any_mapped = self
            .mapping
            .as_ref()
            .is_some_and(|mapping| mapping.bindings.iter().any(Option::is_some));
        let selection_mapped = self.selection.iter().any(|key| self.binding(key).is_some());
        h_flex()
            .gap_2()
            .items_center()
            .when(any_mapped, |this| {
                this.child(
                    Button::new("select-mapped")
                        .small()
                        .label("Select mapped")
                        .on_click(cx.listener(|this, _, _, cx| this.select_mapped(cx))),
                )
            })
            .child(
                Button::new("select-all")
                    .small()
                    .label("Select all keys")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(keyboard) = this.store.read(cx).keyboard().cloned() {
                            this.selection.select_all(&keyboard);
                        }
                        cx.notify();
                    })),
            )
            .when(selection_mapped, |this| {
                this.child(
                    Button::new("remove-control")
                        .small()
                        .label("Remove control")
                        .on_click(cx.listener(|this, _, _, cx| this.assign(None, cx))),
                )
            })
            .when(selected > 0, |this| {
                this.child(
                    Button::new("clear-selection")
                        .small()
                        .label("Discard selection")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.selection.clear();
                            cx.notify();
                        })),
                )
            })
    }

    fn control_groups(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let common = self.common_binding();
        let unavailable = self.selection.is_empty() || self.mapping.is_none();
        let mut groups = v_flex().gap_2();
        for group in &controls::GROUPS {
            let buttons = group.controls.iter().map(|control| {
                let control = *control;
                let (short, long) = usage::control(control);
                let tooltip = match controls::support(control) {
                    Support::Both => SharedString::from(long),
                    Support::XboxOnly => format!("{long} · Xbox only").into(),
                    Support::ClassicOnly => format!("{long} · Classic only").into(),
                };
                Button::new(SharedString::from(format!("control-{control:?}")))
                    .outline()
                    .small()
                    .label(short)
                    .tooltip(tooltip)
                    .chosen(common == Some(control))
                    .disabled(unavailable)
                    .on_click(cx.listener(move |this, _, _, cx| this.assign(Some(control), cx)))
            });
            groups = groups.child(
                h_flex()
                    .gap_3()
                    .items_start()
                    .child(
                        div()
                            .w(px(GROUP_LABEL_WIDTH))
                            .flex_none()
                            .pt_1()
                            .text_xs()
                            .text_color(muted)
                            .child(group.title),
                    )
                    .child(h_flex().flex_1().gap_1().flex_wrap().children(buttons)),
            );
        }
        groups
    }

    fn curve_card(&self, cx: &mut Context<Self>) -> Div {
        let card = panel::section(
            "Analog curve",
            Some("How key travel becomes the analog value.".into()),
            cx,
        );
        let Some(settings) = self.analog else {
            return card;
        };
        let current = curve::preset_of(settings.curve);
        let presets = segmented(
            ButtonGroup::new("curve-presets")
                .compact()
                .children(curve::PRESETS.iter().enumerate().map(|(index, preset)| {
                    Button::new(SharedString::from(format!("curve-preset-{index}")))
                        .flex_auto()
                        .label(preset.name)
                        .chosen(current == Some(index))
                }))
                .on_click(cx.listener(|this, clicks: &Vec<usize>, window, cx| {
                    if let Some(preset) =
                        clicks.first().and_then(|index| curve::PRESETS.get(*index))
                    {
                        let points = preset.curve;
                        this.change_analog(|settings| settings.curve = points, window, cx);
                    }
                })),
        );
        let page = cx.entity().downgrade();
        let handler: chart::Handler = Rc::new(move |input, window, cx| {
            page.update(cx, |page, cx| page.on_chart(input, window, cx))
                .ok();
        });
        let point = settings.curve.get(self.point).copied();
        let travel = point.map_or(0, |point| {
            curve::percent(f32::from(point.travel) / POINT_SCALE)
        });
        let output = point.map_or(0, |point| {
            curve::percent(f32::from(point.output) / POINT_SCALE)
        });
        card.child(panel::field("Presets", presets, cx))
            .child(chart::chart(
                Chart {
                    curve: settings.curve,
                    selected: Some(self.point),
                    markers: Vec::new(),
                    handler: Some(handler),
                },
                cx,
            ))
            .child(panel::field(
                "Point",
                segmented(
                    ButtonGroup::new("curve-point")
                        .children(POINT_NAMES.iter().enumerate().map(|(index, name)| {
                            Button::new(*name)
                                .flex_1()
                                .label(*name)
                                .chosen(index == self.point)
                        }))
                        .on_click(cx.listener(|this, clicks: &Vec<usize>, window, cx| {
                            if let Some(index) = clicks.first() {
                                this.choose_point(*index, window, cx);
                            }
                        })),
                ),
                cx,
            ))
            .child(panel::field(
                "Travel",
                slider_with_value(&self.point_travel, format!("{travel}%"), cx),
                cx,
            ))
            .child(panel::field(
                "Output",
                slider_with_value(&self.point_output, format!("{output}%"), cx),
                cx,
            ))
    }

    fn joystick_card(&self, cx: &mut Context<Self>) -> Div {
        let settings = self.analog;
        let bias = settings.and_then(|settings| settings.left_stick_up_bias);
        panel::section(
            "Joystick",
            Some("How the keys drive the analog sticks.".into()),
            cx,
        )
        .child(panel::row(
            "Use square joystick output",
            Some("Full output on both axes at once; some games disagree.".into()),
            Switch::new("square-joystick")
                .checked(
                    settings
                        .is_some_and(|settings| settings.normalization == Normalization::Square),
                )
                .disabled(settings.is_none())
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
        .child(panel::row(
            "Snappy joystick",
            Some("Of two opposite keys, the deeper one wins.".into()),
            Switch::new("snappy-joystick")
                .checked(settings.is_some_and(|settings| settings.single_direction_axes))
                .disabled(settings.is_none())
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
        .child(panel::row(
            "Enable angle adjustment",
            Some("Shifts the forward diagonal of the left stick.".into()),
            Switch::new("joystick-angle")
                .checked(bias.is_some())
                .disabled(settings.is_none())
                .on_change(cx.listener(|this, checked: &bool, window, cx| {
                    let bias = checked.then_some(DEFAULT_UP_BIAS);
                    this.change_analog(|settings| settings.left_stick_up_bias = bias, window, cx);
                })),
            cx,
        ))
        .when_some(bias, |this, bias| {
            this.child(panel::field(
                "Diagonal angle",
                slider_with_value(&self.up_bias, format!("{bias}%"), cx),
                cx,
            ))
        })
    }

    fn live_curve_card(&self, keyboard: &Keyboard, cx: &mut Context<Self>) -> Div {
        let card = panel::section(
            "Analog output",
            Some("Press mapped keys to see their analog value.".into()),
            cx,
        );
        let Some(settings) = self.analog else {
            return card;
        };
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
                selected: None,
                markers,
                handler: None,
            },
            cx,
        ))
    }

    fn identify_card(&self, cx: &mut Context<Self>) -> Div {
        let running = self
            .started_with
            .is_some_and(|settings| settings.mode != Mode::NoGamepad);
        let card = panel::section(
            "Identify controller",
            Some("Sends a fixed pattern so games can find the controller.".into()),
            cx,
        );
        if !running {
            return card.child(panel::note("Gamepad is off.", cx));
        }
        card.child(panel::row(
            "Send detection pattern",
            None,
            Switch::new("gamepad-detection")
                .checked(self.detecting)
                .on_change(cx.listener(|this, checked: &bool, _, cx| {
                    this.set_detection(*checked, cx);
                })),
            cx,
        ))
    }

    fn configured_mode(&self) -> Mode {
        self.device
            .map_or(Mode::NoGamepad, |settings| settings.mode)
    }
}

fn segmented(group: ButtonGroup) -> ButtonGroup {
    group.outline().w_full()
}

trait Choice {
    fn chosen(self, chosen: bool) -> Self;
}

impl Choice for Button {
    fn chosen(self, chosen: bool) -> Self {
        self.selected(chosen).when(chosen, ButtonVariants::primary)
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
                .text_color(cx.theme().muted_foreground)
                .child(value),
        )
}

fn key_legend(keyboard: &Keyboard, key: usize) -> SharedString {
    keyboard
        .legends
        .get(key)
        .map_or_else(SharedString::default, |action| {
            SharedString::from(usage::legend(*action).into_owned())
        })
}

fn mapping_face(
    keyboard: &Keyboard,
    key: usize,
    binding: Option<Control>,
    mode: Mode,
    mapped: Hsla,
    unsupported: Hsla,
) -> KeyFace {
    let legend = key_legend(keyboard, key);
    match binding {
        Some(control) => KeyFace {
            legend: usage::control(control).0.into(),
            detail: Some(legend),
            fill: Some(
                if mode == Mode::NoGamepad || controls::works_in(control, mode) {
                    mapped
                } else {
                    unsupported
                },
            ),
            ..KeyFace::default()
        },
        None => KeyFace {
            legend,
            ..KeyFace::default()
        },
    }
}

fn test_face(
    keyboard: &Keyboard,
    key: usize,
    binding: Option<Control>,
    travel: Option<u16>,
    curve: Option<curve::Curve>,
) -> KeyFace {
    let Some(control) = binding else {
        return KeyFace {
            legend: key_legend(keyboard, key),
            dimmed: true,
            ..KeyFace::default()
        };
    };
    let level = travel.map(curve::travel_level);
    let value = match (level, curve) {
        (Some(level), Some(curve)) if controls::is_analog(control) => {
            Some(curve::percent(curve::output(curve, level)))
        }
        (Some(level), _) => Some(curve::percent(level)),
        (None, _) => None,
    };
    KeyFace {
        legend: usage::control(control).0.into(),
        detail: value.map(|value| value.to_string().into()),
        travel: level,
        ..KeyFace::default()
    }
}
