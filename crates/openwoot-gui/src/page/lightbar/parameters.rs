use std::ops::RangeInclusive;

use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    component::{ActiveTheme as _, Sizable as _, h_flex, input::Input, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    lightbar::{
        Background, Blink, Custom, Effect, Entry, KeyRect, MAX_LEDS, Meter, Mirror, Operand,
        ProfileIndicator, Range, Signal, Solid, Target, Term, Waveform,
    },
    lighting::Rgb,
};

use super::{
    LightbarPage, Tab, WHITE,
    entry::{
        self, BUILTINS, Kind, Spot, TYPES, above_keys, full_range, lightbar_range, maximum_of,
        number_row,
    },
    number::Field,
    swatch::Swatch,
};
use crate::{
    color_picker::{self, Swatch as ColorSwatch, hsla_of},
    dropdown::{Dropdown, Item},
    page::lighting::{Choice, PARAM_TEXT, choices, stepper},
    panel,
    slider::Slider,
};

const INPUT_WIDTH: f32 = 48.0;
const PLACEHOLDER_TEXT: f32 = 16.0;
const PLACEHOLDER_LINE: f32 = 24.0;
const BOX_RADIUS: f32 = 6.0;
const UNIT_LINE: f32 = 19.2;
const CONTROL_HEIGHT: f32 = 32.0;
const TAB_HEIGHT: f32 = 40.0;
const TYPE_WIDTH: f32 = 280.0;
const SWATCH_SIZE: f32 = 32.0;
const LED_SWATCH: f32 = 24.0;
const FIVE_SECTIONS: u8 = 5;
const BACKGROUNDS: [(&str, Background); 4] = [
    ("Transparent", Background::Transparent),
    ("Fixed", Background::Fixed(entry::color(255, 255, 255))),
    ("Complementing", Background::Contrast),
    ("Darker", Background::DimmedProfileColor),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Socket {
    Meter,
    Term(Spot),
    Operand(Spot),
}

impl LightbarPage {
    pub(super) fn parameters_card(&self, cx: &Context<Self>) -> Option<Div> {
        self.settings.as_ref()?;
        let card = panel::section("Effect Parameters", None, cx).p_6();
        let Some(entry) = self.chosen_entry() else {
            return Some(
                card.child(
                    div()
                        .text_size(px(PLACEHOLDER_TEXT))
                        .line_height(px(PLACEHOLDER_LINE))
                        .text_color(cx.theme().muted_foreground)
                        .child("Select an effect to change its parameters."),
                ),
            );
        };
        if self.tab == Tab::Presets {
            return Some(card.child(parameter_box(cx).children(self.kind_rows(entry, cx))));
        }
        let temporary = entry.condition.is_some();
        let define = self.define || !temporary;
        let body = if define {
            self.define_effect(entry, cx)
        } else {
            self.conditions(entry, cx)
        };
        Some(
            card.child(
                v_flex()
                    .w_full()
                    .child(Self::creator_tabs(temporary, define, cx))
                    .child(div().w_full().p_4().child(body)),
            ),
        )
    }

    fn creator_tabs(temporary: bool, define: bool, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let tabs: &[(&str, bool)] = if temporary {
            &[("Define Effect", true), ("Conditions", false)]
        } else {
            &[("Define Effect", true)]
        };
        h_flex()
            .w_full()
            .gap_2()
            .border_b_1()
            .border_color(theme.border)
            .children(tabs.iter().map(|(label, defines)| {
                let selected = *defines == define;
                let defines = *defines;
                div()
                    .id(SharedString::from(format!("creator-tab-{label}")))
                    .h(px(TAB_HEIGHT))
                    .px_4()
                    .flex()
                    .items_center()
                    .rounded_t(px(BOX_RADIUS))
                    .text_size(px(PLACEHOLDER_TEXT))
                    .line_height(px(PLACEHOLDER_LINE))
                    .cursor_pointer()
                    .map(|this| {
                        if selected {
                            this.bg(theme.secondary).text_color(theme.foreground)
                        } else {
                            this.text_color(theme.group_box_foreground)
                                .hover(|style| style.bg(theme.accent))
                        }
                    })
                    .child(*label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.define = defines;
                        cx.notify();
                    }))
            }))
    }

    pub(super) fn number(&self, field: Field, cx: &Context<Self>) -> Div {
        h_flex()
            .h(px(CONTROL_HEIGHT))
            .gap_2()
            .items_center()
            .child(
                div()
                    .w(px(INPUT_WIDTH))
                    .when_some(self.field(field), |this, state| {
                        this.child(Input::new(state).small())
                    }),
            )
            .when(!field.unit().is_empty(), |this| {
                this.child(
                    div()
                        .text_size(px(PARAM_TEXT))
                        .line_height(px(UNIT_LINE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(field.unit()),
                )
            })
    }

    pub(super) fn signal_menu(
        &self,
        current: Option<Signal>,
        socket: Socket,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        let host_signals = self
            .capabilities(cx)
            .is_some_and(|capabilities| capabilities.host_signals);
        let host = match current {
            Some(Signal::Host(id)) => id,
            _ => self
                .host_ids
                .as_ref()
                .and_then(|ids| ids.first().copied())
                .unwrap_or_default(),
        };
        let (id, value) = match socket {
            Socket::Meter => ("signal-meter".to_owned(), false),
            Socket::Term(spot) => (format!("signal-term-{}-{}", spot.group, spot.term), false),
            Socket::Operand(spot) => (format!("signal-operand-{}-{}", spot.group, spot.term), true),
        };
        let choices: Vec<Option<Signal>> = value
            .then_some(None)
            .into_iter()
            .chain(
                BUILTINS
                    .iter()
                    .map(|builtin| Some(Signal::Builtin(*builtin))),
            )
            .chain(host_signals.then_some(Some(Signal::Host(host))))
            .collect();
        let selected = choices
            .iter()
            .position(|choice| same_choice(*choice, current));
        let items: Vec<Item> = choices
            .iter()
            .map(|choice| {
                Item::new(match choice {
                    None => "Value".to_owned(),
                    Some(Signal::Builtin(builtin)) => entry::builtin_name(*builtin).to_owned(),
                    Some(Signal::Host(_)) => "Host signal".to_owned(),
                })
            })
            .collect();
        Dropdown::new(SharedString::from(id))
            .small()
            .items(items)
            .selected(selected)
            .on_select(move |index, window, cx| {
                let Some(choice) = choices.get(index).copied() else {
                    return;
                };
                page.update(cx, |page, cx| page.set_signal(socket, choice, window, cx))
                    .ok();
            })
    }

    fn define_effect(&self, entry: &Entry, cx: &Context<Self>) -> Div {
        let temporary = entry.condition.is_some();
        let family = Kind::of(&entry.effect).family();
        let page = cx.entity().downgrade();
        let rows = self.kind_rows(entry, cx);
        let settings = parameter_box(cx)
            .child(panel::field(
                "Display",
                choices(
                    "entry-display",
                    ["Temporarily", "Constantly"],
                    Some(usize::from(!temporary)),
                    Choice::Outlined,
                    cx.listener(|this, index: &usize, window, cx| {
                        let temporary = *index == 0;
                        this.edit_chosen(
                            |entry| {
                                if entry.condition.is_some() != temporary {
                                    entry.condition = temporary.then(entry::default_condition);
                                }
                            },
                            window,
                            cx,
                        );
                        this.sync_entry_controls(window, cx);
                    }),
                    cx,
                ),
                cx,
            ))
            .child(panel::field(
                "Effect Type",
                div().w(px(TYPE_WIDTH)).child(
                    Dropdown::new("entry-type")
                        .small()
                        .items(TYPES.iter().map(|kind| Item::new(kind.name())))
                        .selected(TYPES.iter().position(|kind| *kind == family))
                        .on_select(move |index, window, cx| {
                            let Some(kind) = TYPES.get(index).copied() else {
                                return;
                            };
                            page.update(cx, |page, cx| page.change_kind(kind, window, cx))
                                .ok();
                        }),
                ),
                cx,
            ))
            .children(rows);
        v_flex()
            .w_full()
            .gap_4()
            .child(panel::description(
                "Choose whether the effect is constant or temporary and how it looks.",
                cx,
            ))
            .child(settings)
    }

    fn kind_rows(&self, entry: &Entry, cx: &Context<Self>) -> Vec<Div> {
        match &entry.effect {
            Effect::Solid(_) | Effect::Custom(_) => self.solid_rows(&entry.effect, cx),
            Effect::Blink(blink) => self.pulse_rows(*blink, cx),
            Effect::Meter(meter) => self.meter_rows(meter, cx),
            Effect::Mirror(mirror) => self.sync_rows(*mirror, cx),
            Effect::ProfileIndicator(indicator) => self.indicator_rows(*indicator, cx),
        }
    }

    fn solid_rows(&self, effect: &Effect, cx: &Context<Self>) -> Vec<Div> {
        let multiple = matches!(effect, Effect::Custom(_));
        let colour = choices(
            "solid-colours",
            ["Single colour", "Multiple colours"],
            Some(usize::from(multiple)),
            Choice::Outlined,
            cx.listener(|this, index: &usize, window, cx| {
                let multiple = *index == 1;
                this.convert_solid(multiple, window, cx);
            }),
            cx,
        );
        let swatches = match effect {
            Effect::Custom(custom) => self.led_swatches(custom, cx),
            _ => self.swatch("lightbar-main", Swatch::Main),
        };
        let mut rows = vec![panel::field(
            "Colour",
            v_flex().gap_2().child(colour).child(swatches),
            cx,
        )];
        match effect {
            Effect::Solid(solid) => rows.extend(self.target_rows(solid.target, cx)),
            Effect::Custom(custom) => rows.push(self.range_field(custom.range, cx)),
            _ => {}
        }
        rows
    }

    fn pulse_rows(&self, blink: Blink, cx: &Context<Self>) -> Vec<Div> {
        let mut rows = vec![
            panel::field("Main Colour", self.swatch("pulse-main", Swatch::Main), cx),
            panel::field(
                "Secondary Colour",
                self.swatch("pulse-secondary", Swatch::Second),
                cx,
            ),
            panel::field(
                "Transition",
                choices(
                    "pulse-transition",
                    ["Smooth", "Instant"],
                    Some(usize::from(blink.waveform == Waveform::Square)),
                    Choice::Outlined,
                    cx.listener(|this, index: &usize, window, cx| {
                        let waveform = if *index == 1 {
                            Waveform::Square
                        } else {
                            Waveform::Triangle
                        };
                        this.edit_chosen(
                            |entry| {
                                if let Effect::Blink(blink) = &mut entry.effect {
                                    blink.waveform = waveform;
                                }
                            },
                            window,
                            cx,
                        );
                    }),
                    cx,
                ),
                cx,
            ),
            panel::field(
                "Speed",
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().min_w_0().child(Slider::new(&self.period)))
                    .child(self.number(Field::Period, cx)),
                cx,
            ),
        ];
        rows.extend(self.target_rows(blink.target, cx));
        rows
    }

    fn meter_rows(&self, meter: &Meter, cx: &Context<Self>) -> Vec<Div> {
        let host = matches!(meter.signal, Signal::Host(_));
        vec![
            panel::field(
                "Data Point",
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(self.signal_menu(
                        Some(meter.signal),
                        Socket::Meter,
                        cx,
                    )))
                    .when(host, |this| this.child(self.number(Field::MeterHost, cx))),
                cx,
            ),
            panel::field("Max Value", self.number(Field::Maximum, cx), cx),
            h_flex()
                .w_full()
                .flex_wrap()
                .gap_x_8()
                .gap_y_4()
                .child(div().child(panel::field(
                    "Main Colour",
                    self.swatch("meter-main", Swatch::Main),
                    cx,
                )))
                .child(div().child(panel::field(
                    "Background Colour",
                    self.swatch("meter-background", Swatch::Second),
                    cx,
                )))
                .child(div().child(panel::field(
                    "Blend Colour",
                    self.swatch("meter-blend", Swatch::Third),
                    cx,
                ))),
            panel::field(
                "Direction",
                choices(
                    "meter-direction",
                    ["Left to right", "Right to left"],
                    Some(usize::from(meter.reverse)),
                    Choice::Outlined,
                    cx.listener(|this, index: &usize, window, cx| {
                        let reverse = *index == 1;
                        this.edit_chosen(
                            |entry| {
                                if let Effect::Meter(meter) = &mut entry.effect {
                                    meter.reverse = reverse;
                                }
                            },
                            window,
                            cx,
                        );
                    }),
                    cx,
                ),
                cx,
            ),
            self.range_field(meter.range, cx),
        ]
    }

    fn sync_rows(&self, mirror: Mirror, cx: &Context<Self>) -> Vec<Div> {
        let matrix = self.matrix(cx);
        let selected = if self.custom_source {
            2
        } else if mirror.source == number_row(matrix) {
            0
        } else if mirror.source == above_keys(matrix) {
            1
        } else {
            2
        };
        let mut rows = vec![panel::field(
            "Match with",
            choices(
                "sync-source",
                ["Number row", "Above keys", "Custom keys"],
                Some(selected),
                Choice::Outlined,
                cx.listener(move |this, index: &usize, window, cx| {
                    let matrix = this.matrix(cx);
                    let source = match index {
                        0 => number_row(matrix),
                        1 => above_keys(matrix),
                        _ => {
                            this.custom_source = true;
                            cx.notify();
                            return;
                        }
                    };
                    this.custom_source = false;
                    this.edit_rect(|rect| *rect = source, window, cx);
                }),
                cx,
            ),
            cx,
        )];
        if selected == 2 {
            rows.extend(self.rect_rows(mirror.source, cx));
        }
        rows.push(self.range_field(mirror.range, cx));
        rows
    }

    fn indicator_rows(&self, indicator: ProfileIndicator, cx: &Context<Self>) -> Vec<Div> {
        let full = indicator.background == Background::ProfileColor;
        let mut rows = vec![panel::field(
            "Style",
            choices(
                "indicator-style",
                ["Full", "Divided"],
                Some(usize::from(!full)),
                Choice::Outlined,
                cx.listener(|this, index: &usize, window, cx| {
                    let background = if *index == 0 {
                        Background::ProfileColor
                    } else {
                        Background::DimmedProfileColor
                    };
                    this.set_background(background, window, cx);
                }),
                cx,
            ),
            cx,
        )];
        if !full {
            let selected = BACKGROUNDS.iter().position(|(_, background)| {
                std::mem::discriminant(background) == std::mem::discriminant(&indicator.background)
            });
            let fixed = matches!(indicator.background, Background::Fixed(_));
            rows.push(panel::field(
                "Background Colour",
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(choices(
                        "indicator-background",
                        BACKGROUNDS.map(|(name, _)| name),
                        selected,
                        Choice::Outlined,
                        cx.listener(|this, index: &usize, window, cx| {
                            if let Some((_, background)) = BACKGROUNDS.get(*index) {
                                let background = match background {
                                    Background::Fixed(_) => Background::Fixed(
                                        this.picker(Swatch::Main)
                                            .and_then(|picker| picker.read(cx).value())
                                            .unwrap_or(WHITE)
                                            .opaque(),
                                    ),
                                    other => *other,
                                };
                                this.set_background(background, window, cx);
                            }
                        }),
                        cx,
                    ))
                    .when(fixed, |this| {
                        this.child(self.swatch("indicator-fixed", Swatch::Main))
                    }),
                cx,
            ));
            let sections = indicator.segments;
            let mut labels = vec!["Number of profiles".to_owned(), "Five sections".to_owned()];
            let selected = match sections {
                1 => 0,
                FIVE_SECTIONS => 1,
                other => {
                    labels.push(format!("{other} sections"));
                    2
                }
            };
            rows.push(panel::field(
                "Divide into",
                choices(
                    "indicator-sections",
                    labels,
                    Some(selected),
                    Choice::Outlined,
                    cx.listener(|this, index: &usize, window, cx| {
                        let segments = match index {
                            0 => 1,
                            1 => FIVE_SECTIONS,
                            _ => return,
                        };
                        this.edit_chosen(
                            |entry| {
                                if let Effect::ProfileIndicator(indicator) = &mut entry.effect {
                                    indicator.segments = segments;
                                }
                            },
                            window,
                            cx,
                        );
                    }),
                    cx,
                ),
                cx,
            ));
        }
        rows.push(self.range_field(indicator.range, cx));
        rows
    }

    fn target_rows(&self, target: Target, cx: &Context<Self>) -> Vec<Div> {
        let on_keys = matches!(target, Target::Keys(_));
        let mut rows = vec![panel::field(
            "Show on",
            choices(
                "effect-target",
                ["Light Indicator", "Keys"],
                Some(usize::from(on_keys)),
                Choice::Outlined,
                cx.listener(move |this, index: &usize, window, cx| {
                    let keys = *index == 1;
                    if keys != on_keys {
                        this.retarget(keys, window, cx);
                    }
                }),
                cx,
            ),
            cx,
        )];
        match target {
            Target::Lightbar(range) => rows.push(self.range_field(range, cx)),
            Target::Keys(rect) => rows.extend(self.rect_rows(rect, cx)),
        }
        rows
    }

    fn range_field(&self, range: Range, cx: &Context<Self>) -> Div {
        panel::field_with_help(
            "LED Range",
            "Choose which part of the Light Indicator shows the effect.",
            h_flex().child(self.led_range(range, cx)),
            cx,
        )
    }

    fn swatch(&self, id: &'static str, swatch: Swatch) -> Div {
        div().when_some(self.picker(swatch), |this, picker| {
            this.child(ColorSwatch::new(id, picker).size(px(SWATCH_SIZE)))
        })
    }

    fn led_swatches(&self, custom: &Custom, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let leds = self.leds(cx);
        let Some(picker) = self.picker(Swatch::Led).cloned() else {
            return div();
        };
        h_flex().gap_1().flex_wrap().children((0..leds).map(|led| {
            let inside = (custom.range.first..=custom.range.last).contains(&led);
            let color = custom
                .colors
                .get(usize::from(led))
                .copied()
                .unwrap_or(Rgb::default());
            if !inside {
                return div()
                    .size(px(LED_SWATCH))
                    .rounded_full()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.group_box)
                    .into_any_element();
            }
            let ring = if led == self.led {
                theme.primary
            } else {
                theme.border
            };
            div()
                .id(SharedString::from(format!("led-colour-{led}")))
                .size(px(LED_SWATCH))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| this.pick_led(led, window, cx)),
                )
                .child(
                    color_picker::popover(
                        SharedString::from(format!("led-popover-{led}")),
                        &picker,
                    )
                    .trigger(move |_, _| {
                        div()
                            .size(px(LED_SWATCH))
                            .rounded_full()
                            .border_2()
                            .border_color(ring)
                            .bg(hsla_of(color))
                            .into_any_element()
                    }),
                )
                .into_any_element()
        }))
    }

    fn rect_rows(&self, rect: KeyRect, cx: &Context<Self>) -> Vec<Div> {
        let matrix = self.matrix(cx);
        let last_row = matrix.rows.saturating_sub(1);
        let last_column = matrix.columns.saturating_sub(1);
        let corner = |id: &'static str,
                      value: u8,
                      bounds: RangeInclusive<u8>,
                      name: &str,
                      set: fn(&mut KeyRect, u8)| {
            stepper(
                id,
                value,
                bounds,
                format!("{name} {}", u16::from(value) + 1),
                cx.listener(move |this, value: &u8, window, cx| {
                    let value = *value;
                    this.edit_rect(move |rect| set(rect, value), window, cx);
                }),
            )
        };
        vec![
            panel::toggle(
                "From",
                None,
                h_flex()
                    .gap_1()
                    .child(corner(
                        "rect-first-row",
                        rect.first.row,
                        0..=rect.last.row,
                        "Row",
                        |rect, row| rect.first.row = row,
                    ))
                    .child(corner(
                        "rect-first-column",
                        rect.first.column,
                        0..=rect.last.column,
                        "Key",
                        |rect, column| rect.first.column = column,
                    )),
                cx,
            ),
            panel::toggle(
                "To",
                None,
                h_flex()
                    .gap_1()
                    .child(corner(
                        "rect-last-row",
                        rect.last.row,
                        rect.first.row..=last_row,
                        "Row",
                        |rect, row| rect.last.row = row,
                    ))
                    .child(corner(
                        "rect-last-column",
                        rect.last.column,
                        rect.first.column..=last_column,
                        "Key",
                        |rect, column| rect.last.column = column,
                    )),
                cx,
            ),
        ]
    }

    fn pick_led(&mut self, led: u8, window: &mut Window, cx: &mut Context<Self>) {
        self.led = led;
        self.sync_entry_controls(window, cx);
        cx.notify();
    }

    fn change_kind(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        let matrix = self.matrix(cx);
        let Some(current) = self.chosen_entry().map(|entry| entry.effect.clone()) else {
            return;
        };
        if Kind::of(&current).family() == kind {
            return;
        }
        let mut effect = kind.effect(matrix);
        if let Some(range) = lightbar_range(&current) {
            set_range(&mut effect, range);
        }
        self.edit_chosen(|entry| entry.effect = effect, window, cx);
        self.sync_entry_controls(window, cx);
    }

    fn convert_solid(&mut self, multiple: bool, window: &mut Window, cx: &mut Context<Self>) {
        let leds = usize::from(self.leds(cx)).min(MAX_LEDS);
        let matrix = self.matrix(cx);
        let Some(current) = self.chosen_entry().map(|entry| entry.effect.clone()) else {
            return;
        };
        let converted = match (multiple, &current) {
            (true, Effect::Solid(solid)) => Effect::Custom(Custom {
                colors: (0..leds).map(|_| solid.color.rgb()).collect(),
                range: match solid.target {
                    Target::Lightbar(range) => range,
                    Target::Keys(_) => full_range(matrix.leds),
                },
            }),
            (false, Effect::Custom(custom)) => Effect::Solid(Solid {
                color: custom
                    .colors
                    .get(usize::from(custom.range.first))
                    .copied()
                    .unwrap_or(WHITE)
                    .opaque(),
                target: Target::Lightbar(custom.range),
            }),
            _ => return,
        };
        self.led = 0;
        self.edit_chosen(|entry| entry.effect = converted, window, cx);
        self.sync_entry_controls(window, cx);
    }

    fn set_signal(
        &mut self,
        socket: Socket,
        signal: Option<Signal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_chosen(
            |entry| match (socket, signal) {
                (Socket::Meter, Some(signal)) => {
                    if let Effect::Meter(meter) = &mut entry.effect
                        && meter.signal != signal
                    {
                        meter.signal = signal;
                        meter.maximum = maximum_of(signal);
                    }
                }
                (Socket::Term(spot), Some(signal)) => {
                    if let Some(term) = entry::term_mut(entry, spot) {
                        entry::set_term_signal(term, signal);
                    }
                }
                (Socket::Operand(spot), _) => {
                    if let Some(Term::Compare(comparison)) = entry::term_mut(entry, spot) {
                        comparison.operand = match (signal, comparison.operand) {
                            (Some(signal), _) => Operand::Signal(signal),
                            (None, Operand::Constant(value)) => Operand::Constant(value),
                            (None, Operand::Signal(_)) => Operand::Constant(1.0),
                        };
                    }
                }
                (Socket::Meter | Socket::Term(_), None) => {}
            },
            window,
            cx,
        );
        self.sync_entry_controls(window, cx);
    }

    fn set_background(
        &mut self,
        background: Background,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_chosen(
            |entry| {
                if let Effect::ProfileIndicator(indicator) = &mut entry.effect {
                    indicator.background = background;
                }
            },
            window,
            cx,
        );
        self.sync_entry_controls(window, cx);
    }

    fn retarget(&mut self, keys: bool, window: &mut Window, cx: &mut Context<Self>) {
        let matrix = self.matrix(cx);
        let target = if keys {
            Target::Keys(number_row(matrix))
        } else {
            Target::Lightbar(full_range(matrix.leds))
        };
        self.edit_chosen(
            |entry| match &mut entry.effect {
                Effect::Blink(Blink {
                    target: current, ..
                })
                | Effect::Solid(Solid {
                    target: current, ..
                }) => *current = target,
                _ => {}
            },
            window,
            cx,
        );
        self.sync_entry_controls(window, cx);
    }

    fn edit_rect(
        &mut self,
        change: impl FnOnce(&mut KeyRect),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_chosen(
            |entry| {
                if let Some(rect) = rect_of(&mut entry.effect) {
                    change(rect);
                }
            },
            window,
            cx,
        );
    }
}

const fn set_range(effect: &mut Effect, range: Range) {
    match effect {
        Effect::Meter(Meter { range: current, .. })
        | Effect::ProfileIndicator(ProfileIndicator { range: current, .. })
        | Effect::Mirror(Mirror { range: current, .. })
        | Effect::Custom(Custom { range: current, .. })
        | Effect::Blink(Blink {
            target: Target::Lightbar(current),
            ..
        })
        | Effect::Solid(Solid {
            target: Target::Lightbar(current),
            ..
        }) => *current = range,
        Effect::Blink(_) | Effect::Solid(_) => {}
    }
}

const fn rect_of(effect: &mut Effect) -> Option<&mut KeyRect> {
    match effect {
        Effect::Mirror(Mirror { source: rect, .. })
        | Effect::Blink(Blink {
            target: Target::Keys(rect),
            ..
        })
        | Effect::Solid(Solid {
            target: Target::Keys(rect),
            ..
        }) => Some(rect),
        _ => None,
    }
}

fn parameter_box(cx: &Context<LightbarPage>) -> Div {
    v_flex()
        .w_full()
        .gap_6()
        .p_4()
        .rounded(px(BOX_RADIUS))
        .bg(cx.theme().tab_active)
}

fn same_choice(choice: Option<Signal>, current: Option<Signal>) -> bool {
    match (choice, current) {
        (None, None) | (Some(Signal::Host(_)), Some(Signal::Host(_))) => true,
        (Some(Signal::Builtin(builtin)), Some(Signal::Builtin(other))) => builtin == other,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::lightbar::entry::Matrix;

    #[test]
    fn a_new_effect_type_keeps_the_led_range() {
        let matrix = Matrix {
            leds: 10,
            rows: 6,
            columns: 17,
        };
        let range = Range { first: 2, last: 6 };
        for kind in TYPES.into_iter().chain([Kind::Custom]) {
            let mut effect = kind.effect(matrix);
            set_range(&mut effect, range);
            assert_eq!(lightbar_range(&effect), Some(range), "{kind:?}");
        }
    }
}
