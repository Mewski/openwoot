use std::ops::RangeInclusive;

use gpui_kit::{
    Context, Div, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
    component::{
        ActiveTheme as _, Sizable as _,
        button::{Button, ButtonVariants as _},
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
    LightbarPage,
    entry::{self, BUILTINS, KINDS, Kind, Spot, full_range, maximum_of, number_row},
    number::Field,
    swatch::Swatch,
};
use crate::{
    page::lighting::{hsla_of, opaque_of, segmented, select, stepper, sunken, tile, wide_slider},
    panel,
};

const INPUT_WIDTH: f32 = 120.0;
const LED_CELL: f32 = 20.0;
const METER_DIRECTIONS: [(bool, &str); 2] = [(false, "→"), (true, "←")];
const TRANSITIONS: [(Waveform, &str); 2] = [
    (Waveform::Square, "Instant"),
    (Waveform::Triangle, "Smooth"),
];
const TARGETS: [&str; 2] = ["Light bar", "Keys"];
const BACKGROUNDS: [(&str, Background); 5] = [
    ("Full", Background::ProfileColor),
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
    pub(super) fn effect_card(&self, cx: &Context<Self>) -> Option<Div> {
        let entry = self.chosen_entry()?;
        let kind = Kind::of(&entry.effect);
        Some(
            panel::section("Effect", None, cx)
                .child(panel::field("Type", self.kind_tiles(kind, cx), cx))
                .children(self.effect_rows(entry, cx)),
        )
    }

    fn kind_tiles(&self, current: Kind, cx: &Context<Self>) -> Div {
        let matrix = self.matrix(cx);
        sunken(cx).child(
            div()
                .grid()
                .grid_cols(2)
                .gap_1()
                .children(KINDS.iter().enumerate().map(|(index, kind)| {
                    let kind = *kind;
                    let effect = kind.effect(matrix);
                    tile(
                        format!("lightbar-kind-{index}"),
                        kind.icon(),
                        kind.name(),
                        kind.description(),
                        kind == current,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if kind != current {
                            let effect = effect.clone();
                            this.edit_chosen(|entry| entry.effect = effect, window, cx);
                            this.sync_entry_controls(window, cx);
                        }
                    }))
                })),
        )
    }

    fn effect_rows(&self, entry: &Entry, cx: &Context<Self>) -> Vec<Div> {
        match &entry.effect {
            Effect::Meter(meter) => self.meter_rows(meter, cx),
            Effect::ProfileIndicator(indicator) => self.indicator_rows(*indicator, cx),
            Effect::Mirror(mirror) => {
                let mut rows = self.rect_rows(mirror.source, cx);
                rows.extend(self.range_row(cx));
                rows
            }
            Effect::Blink(blink) => {
                let mut rows = vec![
                    self.color_pair_row(cx),
                    choice_toggle(
                        "Transition",
                        "blink-transition",
                        &TRANSITIONS,
                        blink.waveform,
                        |effect, waveform| {
                            if let Effect::Blink(blink) = effect {
                                blink.waveform = waveform;
                            }
                        },
                        cx,
                    ),
                    panel::toggle("Period", None, self.number(Field::Period, "ms"), cx),
                ];
                rows.extend(self.target_rows(blink.target, cx));
                rows
            }
            Effect::Solid(solid) => {
                let mut rows = vec![self.color_row("Colour", Swatch::Main, cx)];
                rows.extend(self.target_rows(solid.target, cx));
                rows
            }
            Effect::Custom(custom) => {
                let mut rows = vec![
                    self.custom_strip(custom.colors.as_slice(), cx),
                    panel::toggle(
                        "Colour",
                        None,
                        h_flex()
                            .gap_2()
                            .items_center()
                            .when_some(self.swatch(Swatch::Main), |this, picker| {
                                this.child(ColorPicker::new(picker).small())
                            })
                            .child(
                                Button::new("custom-fill")
                                    .label("Fill all")
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.fill_custom(window, cx);
                                    })),
                            ),
                        cx,
                    ),
                ];
                rows.extend(self.range_row(cx));
                rows
            }
        }
    }

    fn meter_rows(&self, meter: &Meter, cx: &Context<Self>) -> Vec<Div> {
        let mut rows = vec![
            self.signal_field("Data point", meter.signal, Socket::Meter, cx),
            panel::toggle("Maximum", None, self.number(Field::Maximum, ""), cx),
            self.color_row("Colour", Swatch::Main, cx),
            self.optional_color_row(
                "Background",
                None,
                Swatch::Second,
                meter.background.is_some(),
                cx,
            ),
            self.optional_color_row(
                "Blend",
                Some("Fades from this colour at the start of the bar."),
                Swatch::Third,
                meter.gradient_start.is_some(),
                cx,
            ),
            choice_toggle(
                "Direction",
                "meter-direction",
                &METER_DIRECTIONS,
                meter.reverse,
                |effect, reverse| {
                    if let Effect::Meter(meter) = effect {
                        meter.reverse = reverse;
                    }
                },
                cx,
            ),
        ];
        rows.extend(self.range_row(cx));
        rows
    }

    fn indicator_rows(&self, indicator: ProfileIndicator, cx: &Context<Self>) -> Vec<Div> {
        let leds = self.leds(cx).max(1);
        let fixed = matches!(indicator.background, Background::Fixed(_));
        let mut rows = vec![panel::field(
            "Background",
            h_flex()
                .w_full()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Self::background_menu(indicator.background, cx)),
                )
                .when(fixed, |this| {
                    this.when_some(self.swatch(Swatch::Main), |this, picker| {
                        this.child(ColorPicker::new(picker).small())
                    })
                }),
            cx,
        )];
        rows.push(panel::toggle(
            "Sections",
            Some("At least this many; 1 divides by the profile count.".into()),
            stepper(
                "indicator-segments",
                indicator.segments,
                1..=leds,
                indicator.segments.to_string(),
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
        rows.extend(self.range_row(cx));
        rows
    }

    fn background_menu(current: Background, cx: &Context<Self>) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        let label = entry::background_name(current);
        select("indicator-background", label).dropdown_menu(move |menu, _, _| {
            BACKGROUNDS.iter().fold(menu, |menu, (name, background)| {
                let page = page.clone();
                let background = *background;
                let checked = *name == label;
                menu.item(PopupMenuItem::new(*name).checked(checked).on_click(
                    move |_, window, cx| {
                        if !checked {
                            page.update(cx, |page, cx| {
                                page.set_background(background, window, cx);
                            })
                            .ok();
                        }
                    },
                ))
            })
        })
    }

    fn target_rows(&self, target: Target, cx: &Context<Self>) -> Vec<Div> {
        let on_keys = matches!(target, Target::Keys(_));
        let mut rows = vec![panel::toggle(
            "Target",
            None,
            segmented(
                "effect-target",
                TARGETS,
                usize::from(on_keys),
                cx.listener(move |this, index: &usize, window, cx| {
                    let keys = *index == 1;
                    if keys != on_keys {
                        this.retarget(keys, window, cx);
                    }
                }),
            ),
            cx,
        )];
        match target {
            Target::Lightbar(_) => rows.extend(self.range_row(cx)),
            Target::Keys(rect) => rows.extend(self.rect_rows(rect, cx)),
        }
        rows
    }

    fn range_row(&self, cx: &Context<Self>) -> Option<Div> {
        let range = self.range.as_ref()?;
        let value = range.state.read(cx).value();
        let readout = format!(
            "{}–{}",
            u16::from(super::led_of(value.start())) + 1,
            u16::from(super::led_of(value.end())) + 1
        );
        Some(panel::field(
            "LEDs",
            wide_slider(&range.state, readout, cx),
            cx,
        ))
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
                "First key",
                None,
                h_flex()
                    .gap_1()
                    .child(corner(
                        "rect-first-row",
                        rect.first.row,
                        0..=rect.last.row,
                        "Row",
                        |rect, row| {
                            rect.first.row = row;
                        },
                    ))
                    .child(corner(
                        "rect-first-column",
                        rect.first.column,
                        0..=rect.last.column,
                        "Col",
                        |rect, column| rect.first.column = column,
                    )),
                cx,
            ),
            panel::toggle(
                "Last key",
                None,
                h_flex()
                    .gap_1()
                    .child(corner(
                        "rect-last-row",
                        rect.last.row,
                        rect.first.row..=last_row,
                        "Row",
                        |rect, row| {
                            rect.last.row = row;
                        },
                    ))
                    .child(corner(
                        "rect-last-column",
                        rect.last.column,
                        rect.first.column..=last_column,
                        "Col",
                        |rect, column| rect.last.column = column,
                    )),
                cx,
            ),
        ]
    }

    pub(super) fn signal_field(
        &self,
        label: &'static str,
        signal: Signal,
        socket: Socket,
        cx: &Context<Self>,
    ) -> Div {
        let detail = match signal {
            Signal::Builtin(builtin) => entry::builtin_detail(builtin),
            Signal::Host(_) => "A value a program on the computer sends by id.",
        };
        let host_field = match socket {
            Socket::Meter => Field::MeterHost,
            Socket::Term(spot) => Field::SignalHost(spot),
            Socket::Operand(spot) => Field::OperandHost(spot),
        };
        panel::field(
            label,
            v_flex()
                .w_full()
                .gap_1p5()
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .items_center()
                        .child(div().flex_1().min_w_0().child(self.signal_menu(
                            Some(signal),
                            socket,
                            cx,
                        )))
                        .when(matches!(signal, Signal::Host(_)), |this| {
                            this.child(self.number(host_field, ""))
                        }),
                )
                .child(panel::note(detail, cx)),
            cx,
        )
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
        let (id, label) = match socket {
            Socket::Meter => ("signal-meter".to_owned(), None),
            Socket::Term(spot) => (format!("signal-term-{}-{}", spot.group, spot.term), None),
            Socket::Operand(spot) => (
                format!("signal-operand-{}-{}", spot.group, spot.term),
                Some("Value"),
            ),
        };
        let shown = current.map_or_else(
            || label.unwrap_or_default().to_owned(),
            |signal| match signal {
                Signal::Builtin(builtin) => entry::builtin_name(builtin).to_owned(),
                Signal::Host(_) => "Host signal".to_owned(),
            },
        );
        select(id, shown).dropdown_menu(move |menu, _, _| {
            let choices = label
                .map(|_| None)
                .into_iter()
                .chain(
                    BUILTINS
                        .iter()
                        .map(|builtin| Some(Signal::Builtin(*builtin))),
                )
                .chain(host_signals.then_some(Some(Signal::Host(host))));
            choices.fold(menu, |menu, choice| {
                let page = page.clone();
                let name = match choice {
                    None => "Value".to_owned(),
                    Some(Signal::Builtin(builtin)) => entry::builtin_name(builtin).to_owned(),
                    Some(Signal::Host(_)) => "Host signal".to_owned(),
                };
                menu.item(
                    PopupMenuItem::new(name)
                        .checked(same_choice(choice, current))
                        .on_click(move |_, window, cx| {
                            page.update(cx, |page, cx| {
                                page.set_signal(socket, choice, window, cx);
                            })
                            .ok();
                        }),
                )
            })
        })
    }

    pub(super) fn number(&self, field: Field, unit: &'static str) -> Div {
        div()
            .w(px(INPUT_WIDTH))
            .when_some(self.field(field), |this, state| {
                this.child(
                    NumberInput::new(state)
                        .stepper(false)
                        .when(!unit.is_empty(), |input| {
                            input.suffix(div().text_xs().child(unit))
                        }),
                )
            })
    }

    fn color_row(&self, label: &'static str, swatch: Swatch, cx: &Context<Self>) -> Div {
        panel::toggle(
            label,
            None,
            div().when_some(self.swatch(swatch), |this, picker| {
                this.child(ColorPicker::new(picker).small())
            }),
            cx,
        )
    }

    fn color_pair_row(&self, cx: &Context<Self>) -> Div {
        panel::toggle(
            "Colours",
            None,
            h_flex()
                .gap_2()
                .when_some(self.swatch(Swatch::Main), |this, picker| {
                    this.child(ColorPicker::new(picker).small())
                })
                .when_some(self.swatch(Swatch::Second), |this, picker| {
                    this.child(ColorPicker::new(picker).small())
                }),
            cx,
        )
    }

    fn optional_color_row(
        &self,
        label: &'static str,
        help: Option<&'static str>,
        swatch: Swatch,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Div {
        let id = match swatch {
            Swatch::Second => "meter-background",
            Swatch::Main | Swatch::Third => "meter-blend",
        };
        panel::toggle(
            label,
            help.map(SharedString::from),
            h_flex()
                .gap_3()
                .items_center()
                .when(enabled, |this| {
                    this.when_some(self.swatch(swatch), |this, picker| {
                        this.child(ColorPicker::new(picker).small())
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
        panel::field(
            "Colours",
            h_flex().flex_wrap().gap_1().children((0..leds).map(|led| {
                let color = colors.get(usize::from(led)).copied().unwrap_or_default();
                div()
                    .id(SharedString::from(format!("custom-led-{led}")))
                    .w(px(LED_CELL))
                    .h(px(LED_CELL))
                    .rounded(theme.radius)
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
        self.sync_entry_controls(window, cx);
    }

    pub(super) fn edit_range(
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

fn choice_toggle<T: Copy + PartialEq + 'static, const N: usize>(
    label: &'static str,
    id: &'static str,
    choices: &'static [(T, &'static str); N],
    current: T,
    change: fn(&mut Effect, T),
    cx: &Context<LightbarPage>,
) -> Div {
    let selected = choices
        .iter()
        .position(|(value, _)| *value == current)
        .unwrap_or(0);
    panel::toggle(
        label,
        None,
        segmented(
            id,
            choices.iter().map(|(_, name)| *name),
            selected,
            cx.listener(move |this, index: &usize, window, cx| {
                if let Some((value, _)) = choices.get(*index) {
                    let value = *value;
                    this.edit_chosen(|entry| change(&mut entry.effect, value), window, cx);
                }
            }),
        ),
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

fn same_choice(choice: Option<Signal>, current: Option<Signal>) -> bool {
    match (choice, current) {
        (None, None) | (Some(Signal::Host(_)), Some(Signal::Host(_))) => true,
        (Some(Signal::Builtin(builtin)), Some(Signal::Builtin(other))) => builtin == other,
        _ => false,
    }
}
