use std::ops::RangeInclusive;

use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    component::{
        ActiveTheme as _, Selectable as _, Sizable as _,
        button::Button,
        color_picker::ColorPicker,
        h_flex,
        input::NumberInput,
        menu::{DropdownMenu as _, PopupMenuItem},
        switch::Switch,
        v_flex,
    },
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
    LightbarPage, Owner,
    entry::{self, BUILTINS, KINDS, Kind, Spot, full_range, maximum_of, number_row},
    number::Field,
    swatch::Swatch,
};
use crate::{
    page::lighting::{hsla_of, opaque_of, stepper},
    panel,
};

const INPUT_WIDTH: f32 = 140.0;
const LED_CELL: f32 = 26.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Socket {
    Meter,
    Term(Spot),
    Operand(Spot),
}

const BACKGROUNDS: [(&str, Background); 5] = [
    ("Full", Background::ProfileColor),
    ("Transparent", Background::Transparent),
    ("Fixed", Background::Fixed(entry::color(255, 255, 255))),
    ("Complementing", Background::Contrast),
    ("Darker", Background::DimmedProfileColor),
];

impl LightbarPage {
    pub(super) fn editor_section(&self, cx: &Context<Self>) -> Div {
        let section = panel::section(
            "Effect parameters",
            Some(
                "Choose the effect type and customise its look; a condition decides when it shows."
                    .into(),
            ),
            cx,
        );
        let (Some(chosen), Some(entry)) = (self.chosen, self.chosen_entry()) else {
            return section.child(panel::note(
                "Select an effect to change its parameters.",
                cx,
            ));
        };
        let kind = Kind::of(&entry.effect);
        let owner = match chosen.owner {
            Owner::Profile => "Profile effect",
            Owner::Global => "Global effect",
        };
        section
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
                                    .child(format!(
                                        "{owner} {}: {}",
                                        chosen.index + 1,
                                        kind.name()
                                    )),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(kind.description()),
                            ),
                    )
                    .child(self.kind_menu(kind, cx)),
            )
            .children(self.effect_rows(entry, cx))
            .child(self.condition_rows(entry, cx))
    }

    fn kind_menu(&self, current: Kind, cx: &Context<Self>) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        let matrix = self.matrix(cx);
        Button::new("effect-type")
            .label("Effect type")
            .small()
            .outline()
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                KINDS.iter().fold(menu, |menu, kind| {
                    let page = page.clone();
                    let effect = kind.effect(matrix);
                    menu.item(
                        PopupMenuItem::new(kind.name())
                            .checked(*kind == current)
                            .on_click(move |_, window, cx| {
                                let effect = effect.clone();
                                page.update(cx, |page, cx| {
                                    page.edit_chosen(|entry| entry.effect = effect, window, cx);
                                    page.sync_entry_controls(window, cx);
                                })
                                .ok();
                            }),
                    )
                })
            })
    }

    fn effect_rows(&self, entry: &Entry, cx: &Context<Self>) -> Vec<Div> {
        match &entry.effect {
            Effect::Meter(meter) => self.meter_rows(meter, cx),
            Effect::ProfileIndicator(indicator) => self.indicator_rows(*indicator, cx),
            Effect::Mirror(mirror) => vec![
                panel::row(
                    "Match with",
                    Some("Pick the keys whose colours the LEDs copy.".into()),
                    Button::new("mirror-number-row")
                        .label("Number row")
                        .xsmall()
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| {
                            let rect = number_row(this.matrix(cx));
                            this.edit_rect(move |current| *current = rect, window, cx);
                        })),
                    cx,
                ),
                self.rect_row(mirror.source, cx),
                self.range_row(mirror.range, cx),
            ],
            Effect::Blink(blink) => self.blink_rows(*blink, cx),
            Effect::Solid(solid) => {
                let mut rows = vec![self.color_row("Colour", Swatch::Main, cx)];
                rows.extend(self.target_rows(solid.target, cx));
                rows
            }
            Effect::Custom(custom) => vec![
                self.custom_strip(custom.colors.as_slice(), cx),
                self.color_row("LED colour", Swatch::Main, cx),
                panel::row(
                    "Fill",
                    None,
                    Button::new("custom-fill")
                        .label("Give every LED this colour")
                        .xsmall()
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| this.fill_custom(window, cx))),
                    cx,
                ),
                self.range_row(custom.range, cx),
            ],
        }
    }

    fn meter_rows(&self, meter: &Meter, cx: &Context<Self>) -> Vec<Div> {
        vec![
            self.signal_row("Data point", meter.signal, Socket::Meter, cx),
            panel::row(
                "Max value",
                Some("The value that fills the whole range.".into()),
                self.number(Field::Maximum),
                cx,
            ),
            self.color_row("Main colour", Swatch::Main, cx),
            self.optional_color_row(
                "Background colour",
                "The colour of the unlit LEDs; without it the effects below show through.",
                Swatch::Second,
                meter.background.is_some(),
                cx,
            ),
            self.optional_color_row(
                "Blend colour",
                "Blends from this colour at the start of the bar to the main colour at its end.",
                Swatch::Third,
                meter.gradient_start.is_some(),
                cx,
            ),
            choice_row(
                "Direction",
                [("Left to right", false), ("Right to left", true)],
                meter.reverse,
                |effect, reverse| {
                    if let Effect::Meter(meter) = effect {
                        meter.reverse = reverse;
                    }
                },
                cx,
            ),
            self.range_row(meter.range, cx),
        ]
    }

    fn indicator_rows(&self, indicator: ProfileIndicator, cx: &Context<Self>) -> Vec<Div> {
        let leds = self.leds(cx).max(1);
        let mut rows = vec![panel::row(
            "Background colour",
            Some(
                "Full paints the whole range in the profile colour; the others mark the \
                 profile's section on this background."
                    .into(),
            ),
            h_flex()
                .flex_wrap()
                .gap_1()
                .children(BACKGROUNDS.map(|(label, background)| {
                    let selected = entry::background_name(indicator.background) == label;
                    Button::new(SharedString::from(format!("indicator-{label}")))
                        .label(label)
                        .xsmall()
                        .outline()
                        .selected(selected)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if !selected {
                                this.set_background(background, window, cx);
                            }
                        }))
                })),
            cx,
        )];
        if matches!(indicator.background, Background::Fixed(_)) {
            rows.push(self.color_row("Fixed colour", Swatch::Main, cx));
        }
        rows.push(panel::row(
            "Divide into",
            Some(
                "At least this many sections; 1 divides the range by the number of profiles."
                    .into(),
            ),
            stepper(
                "indicator-segments",
                indicator.segments,
                1..=leds,
                format!("{} sections", indicator.segments),
                cx.listener(|this, segments: &u8, window, cx| {
                    let segments = *segments;
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
            ),
            cx,
        ));
        rows.push(self.range_row(indicator.range, cx));
        rows
    }

    fn blink_rows(&self, blink: Blink, cx: &Context<Self>) -> Vec<Div> {
        let mut rows = vec![
            self.color_row("Main colour", Swatch::Main, cx),
            self.color_row("Secondary colour", Swatch::Second, cx),
            choice_row(
                "Transition",
                [
                    ("Instant", Waveform::Square),
                    ("Smooth", Waveform::Triangle),
                ],
                blink.waveform,
                |effect, waveform| {
                    if let Effect::Blink(blink) = effect {
                        blink.waveform = waveform;
                    }
                },
                cx,
            ),
            panel::row(
                "Cycle duration",
                Some("Milliseconds for one cycle from the main colour and back.".into()),
                self.number(Field::Period),
                cx,
            ),
        ];
        rows.extend(self.target_rows(blink.target, cx));
        rows
    }

    fn target_rows(&self, target: Target, cx: &Context<Self>) -> Vec<Div> {
        let on_keys = matches!(target, Target::Keys(_));
        let chooser = panel::row(
            "Show on",
            None,
            h_flex()
                .gap_1()
                .children([("Light bar", false), ("Keys", true)].map(|(label, keys)| {
                    Button::new(SharedString::from(format!("target-{label}")))
                        .label(label)
                        .xsmall()
                        .outline()
                        .selected(on_keys == keys)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if on_keys != keys {
                                this.retarget(keys, window, cx);
                            }
                        }))
                })),
            cx,
        );
        let place = match target {
            Target::Lightbar(range) => self.range_row(range, cx),
            Target::Keys(rect) => self.rect_row(rect, cx),
        };
        vec![chooser, place]
    }

    fn range_row(&self, range: Range, cx: &Context<Self>) -> Div {
        let last_led = self.leds(cx).saturating_sub(1);
        panel::row(
            "LED range",
            Some("Choose which part of the Light Indicator shows the effect.".into()),
            h_flex()
                .gap_3()
                .child(stepper(
                    "range-first",
                    range.first,
                    0..=range.last.min(last_led),
                    format!("from LED {}", u16::from(range.first) + 1),
                    cx.listener(|this, first: &u8, window, cx| {
                        let first = *first;
                        this.edit_range(move |range| range.first = first, window, cx);
                    }),
                ))
                .child(stepper(
                    "range-last",
                    range.last,
                    range.first..=last_led,
                    format!("to LED {}", u16::from(range.last) + 1),
                    cx.listener(|this, last: &u8, window, cx| {
                        let last = *last;
                        this.edit_range(move |range| range.last = last, window, cx);
                    }),
                )),
            cx,
        )
    }

    fn rect_row(&self, rect: KeyRect, cx: &Context<Self>) -> Div {
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
        panel::row(
            "Keys",
            Some(
                "The rectangle of the key matrix, from its first corner to its last, counting \
                 from the top left."
                    .into(),
            ),
            v_flex()
                .gap_1()
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().w(px(36.0)).text_sm().child("From"))
                        .child(corner(
                            "rect-first-row",
                            rect.first.row,
                            0..=rect.last.row,
                            "row",
                            |rect, row| rect.first.row = row,
                        ))
                        .child(corner(
                            "rect-first-column",
                            rect.first.column,
                            0..=rect.last.column,
                            "column",
                            |rect, column| rect.first.column = column,
                        )),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().w(px(36.0)).text_sm().child("To"))
                        .child(corner(
                            "rect-last-row",
                            rect.last.row,
                            rect.first.row..=last_row,
                            "row",
                            |rect, row| rect.last.row = row,
                        ))
                        .child(corner(
                            "rect-last-column",
                            rect.last.column,
                            rect.first.column..=last_column,
                            "column",
                            |rect, column| rect.last.column = column,
                        )),
                ),
            cx,
        )
    }

    pub(super) fn signal_row(
        &self,
        label: &'static str,
        signal: Signal,
        socket: Socket,
        cx: &Context<Self>,
    ) -> Div {
        let detail = match signal {
            Signal::Builtin(builtin) => entry::builtin_detail(builtin),
            Signal::Host(_) => "A value a program on the computer sends by its id.",
        };
        let host_field = match socket {
            Socket::Meter => Field::MeterHost,
            Socket::Term(spot) => Field::SignalHost(spot),
            Socket::Operand(spot) => Field::OperandHost(spot),
        };
        panel::row(
            label,
            Some(detail.into()),
            h_flex()
                .gap_2()
                .items_center()
                .child(self.signal_menu(signal, socket, cx))
                .when(matches!(signal, Signal::Host(_)), |this| {
                    this.child(self.number(host_field))
                }),
            cx,
        )
    }

    fn signal_menu(
        &self,
        current: Signal,
        socket: Socket,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        let host_signals = self
            .capabilities(cx)
            .is_some_and(|capabilities| capabilities.host_signals);
        let host = match current {
            Signal::Host(id) => id,
            Signal::Builtin(_) => self
                .host_ids
                .as_ref()
                .and_then(|ids| ids.first().copied())
                .unwrap_or_default(),
        };
        let id = match socket {
            Socket::Meter => "signal-meter".to_owned(),
            Socket::Term(spot) => format!("signal-term-{}-{}", spot.group, spot.term),
            Socket::Operand(spot) => format!("signal-operand-{}-{}", spot.group, spot.term),
        };
        Button::new(SharedString::from(id))
            .label(entry::signal_name(current))
            .small()
            .outline()
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                let choices = BUILTINS
                    .iter()
                    .map(|builtin| Signal::Builtin(*builtin))
                    .chain(host_signals.then_some(Signal::Host(host)));
                choices.fold(menu, |menu, signal| {
                    let page = page.clone();
                    let label = match signal {
                        Signal::Builtin(builtin) => entry::builtin_name(builtin).to_owned(),
                        Signal::Host(_) => "Host signal".to_owned(),
                    };
                    menu.item(
                        PopupMenuItem::new(label)
                            .checked(same_signal(signal, current))
                            .on_click(move |_, window, cx| {
                                page.update(cx, |page, cx| {
                                    page.set_signal(socket, signal, window, cx);
                                })
                                .ok();
                            }),
                    )
                })
            })
    }

    pub(super) fn number(&self, field: Field) -> Div {
        div()
            .w(px(INPUT_WIDTH))
            .when_some(self.field(field), |this, state| {
                this.child(NumberInput::new(state).small())
            })
    }

    fn color_row(&self, label: &'static str, swatch: Swatch, cx: &Context<Self>) -> Div {
        panel::row(
            label,
            None,
            div().when_some(self.swatch(swatch), |this, picker| {
                this.child(ColorPicker::new(picker))
            }),
            cx,
        )
    }

    fn optional_color_row(
        &self,
        label: &'static str,
        description: &'static str,
        swatch: Swatch,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Div {
        let id = match swatch {
            Swatch::Second => "meter-background",
            Swatch::Main | Swatch::Third => "meter-blend",
        };
        panel::row(
            label,
            Some(description.into()),
            h_flex()
                .gap_3()
                .items_center()
                .when(enabled, |this| {
                    this.when_some(self.swatch(swatch), |this, picker| {
                        this.child(ColorPicker::new(picker))
                    })
                })
                .child(Switch::new(id).checked(enabled).on_change(cx.listener(
                    move |this, enabled: &bool, window, cx| {
                        this.toggle_meter_color(swatch, *enabled, window, cx);
                    },
                ))),
            cx,
        )
    }

    fn custom_strip(&self, colors: &[Rgb], cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let leds = self.leds(cx);
        panel::row(
            "LEDs",
            Some("Click an LED to choose its colour.".into()),
            h_flex().gap_1().children((0..leds).map(|led| {
                let color = colors.get(usize::from(led)).copied().unwrap_or_default();
                div()
                    .id(SharedString::from(format!("custom-led-{led}")))
                    .w(px(LED_CELL))
                    .h(px(LED_CELL))
                    .rounded(px(4.0))
                    .border_2()
                    .border_color(if led == self.led {
                        theme.primary
                    } else {
                        theme.border
                    })
                    .bg(hsla_of(color))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.led = led;
                        this.sync_entry_controls(window, cx);
                        cx.notify();
                    }))
            })),
            cx,
        )
    }

    fn set_signal(
        &mut self,
        socket: Socket,
        signal: Signal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_chosen(
            |entry| match socket {
                Socket::Meter => {
                    if let Effect::Meter(meter) = &mut entry.effect
                        && meter.signal != signal
                    {
                        meter.signal = signal;
                        meter.maximum = maximum_of(signal);
                    }
                }
                Socket::Term(spot) => {
                    if let Some(term) = entry::term_mut(entry, spot) {
                        entry::set_term_signal(term, signal);
                    }
                }
                Socket::Operand(spot) => {
                    if let Some(Term::Compare(comparison)) = entry::term_mut(entry, spot) {
                        comparison.operand = Operand::Signal(signal);
                    }
                }
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

    fn toggle_meter_color(
        &mut self,
        swatch: Swatch,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let color = self
            .swatch(swatch)
            .and_then(|picker| picker.read(cx).value())
            .map_or(entry::color(255, 255, 255), opaque_of);
        let value = enabled.then_some(color);
        self.edit_chosen(
            |entry| {
                if let Effect::Meter(meter) = &mut entry.effect {
                    match swatch {
                        Swatch::Second => meter.background = value,
                        Swatch::Third => meter.gradient_start = value,
                        Swatch::Main => {}
                    }
                }
            },
            window,
            cx,
        );
        self.sync_entry_controls(window, cx);
    }

    fn fill_custom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let leds = usize::from(self.leds(cx)).min(MAX_LEDS);
        let color = self
            .swatch(Swatch::Main)
            .and_then(|picker| picker.read(cx).value())
            .map_or(entry::color(255, 255, 255), opaque_of)
            .rgb();
        self.edit_chosen(
            |entry| {
                if let Effect::Custom(custom) = &mut entry.effect {
                    custom.colors = (0..leds).map(|_| color).collect();
                }
            },
            window,
            cx,
        );
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
    }

    fn edit_range(
        &mut self,
        change: impl FnOnce(&mut Range),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_chosen(
            |entry| {
                if let Some(range) = range_of(&mut entry.effect) {
                    change(range);
                }
            },
            window,
            cx,
        );
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

fn choice_row<T: Copy + PartialEq + 'static>(
    label: &'static str,
    choices: [(&'static str, T); 2],
    current: T,
    change: fn(&mut Effect, T),
    cx: &Context<LightbarPage>,
) -> Div {
    panel::row(
        label,
        None,
        h_flex().gap_1().children(choices.map(|(name, value)| {
            Button::new(SharedString::from(format!("{label}-{name}")))
                .label(name)
                .xsmall()
                .outline()
                .selected(current == value)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit_chosen(|entry| change(&mut entry.effect, value), window, cx);
                }))
        })),
        cx,
    )
}

const fn range_of(effect: &mut Effect) -> Option<&mut Range> {
    match effect {
        Effect::Meter(Meter { range, .. })
        | Effect::ProfileIndicator(ProfileIndicator { range, .. })
        | Effect::Mirror(Mirror { range, .. })
        | Effect::Custom(Custom { range, .. })
        | Effect::Blink(Blink {
            target: Target::Lightbar(range),
            ..
        })
        | Effect::Solid(Solid {
            target: Target::Lightbar(range),
            ..
        }) => Some(range),
        Effect::Blink(_) | Effect::Solid(_) => None,
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

fn same_signal(signal: Signal, other: Signal) -> bool {
    match (signal, other) {
        (Signal::Host(_), Signal::Host(_)) => true,
        (Signal::Builtin(builtin), Signal::Builtin(other)) => builtin == other,
        _ => false,
    }
}
