use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
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
use openwoot_protocol::{
    advanced::{
        self, Behaviour, DynamicKeystroke, Interrupt, ModTap, Resolution, Socd, StrokeEvent,
    },
    keys::Action,
};

use super::{
    ActionSlot, AdvancedPage,
    kind::{self, Choice, Kind, Point, STROKE_EVENTS},
    rules,
};
use crate::{panel, store::Keyboard, usage};

const SLOT_WIDTH: f32 = 220.0;
const CELL_WIDTH: f32 = 110.0;
const SLIDER_WIDTH: f32 = 260.0;

impl AdvancedPage {
    pub(super) fn editor_section(
        &self,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let entry = *self.selected_entry()?;
        let kind = Kind::of(&entry.behaviour);
        let keys: Vec<String> = rules::bindings(&entry)
            .map(|(key, _)| self.index_name(key))
            .collect();
        let title = format!(
            "{} · {} · {}",
            kind.title(),
            keys.join(" + "),
            usage::layer_name(entry.layer)
        );
        let body = match entry.behaviour {
            Behaviour::DynamicKeystroke(keystroke) => {
                self.keystroke_editor(&entry, keystroke, keyboard, cx)
            }
            Behaviour::ModTap(mod_tap) => self.mod_tap_editor(mod_tap, cx),
            Behaviour::Toggle(action) => self.toggle_editor(action, cx),
            Behaviour::DepthPriority(pair) => {
                let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
                Self::depth_editor(&first, &second, pair.both_bottomed_out, cx)
            }
            Behaviour::Socd(pair) => {
                let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
                Self::socd_editor(&first, &second, pair, cx)
            }
        };
        let picker = kind.owns_actions().then(|| self.picker.clone());
        let tester = self.tester_panel(&entry, cx);
        Some(
            panel::section(title, Some(kind.description().into()), cx)
                .child(body)
                .children(picker)
                .child(tester)
                .child(
                    h_flex().justify_end().child(
                        Button::new("advanced-delete")
                            .small()
                            .danger()
                            .icon(Catalog::Trash)
                            .label("Delete")
                            .on_click(cx.listener(move |_, _, window, cx| {
                                Self::confirm_delete(entry, window, cx);
                            })),
                    ),
                ),
        )
    }

    pub(super) fn set_stroke_event(
        &mut self,
        stroke: usize,
        point: Point,
        event: StrokeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_selected(
            |entry| {
                if let Behaviour::DynamicKeystroke(keystroke) = &mut entry.behaviour
                    && let Some(stroke) = keystroke.strokes.get_mut(stroke)
                {
                    point.set(stroke, event);
                }
            },
            window,
            cx,
        );
    }

    fn set_interrupt(&mut self, interrupt: Interrupt, window: &mut Window, cx: &mut Context<Self>) {
        self.change_selected(
            |entry| {
                if let Behaviour::ModTap(mod_tap) = &mut entry.behaviour {
                    mod_tap.interrupt = interrupt;
                }
            },
            window,
            cx,
        );
    }

    fn set_resolution(
        &mut self,
        resolution: Resolution,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_selected(
            |entry| match &mut entry.behaviour {
                Behaviour::DepthPriority(pair) => pair.both_bottomed_out = resolution,
                Behaviour::Socd(pair) => pair.resolution = resolution,
                Behaviour::DynamicKeystroke(_) | Behaviour::ModTap(_) | Behaviour::Toggle(_) => {}
            },
            window,
            cx,
        );
    }

    fn set_both_when_bottomed_out(
        &mut self,
        both: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_selected(
            |entry| {
                if let Behaviour::Socd(pair) = &mut entry.behaviour {
                    pair.both_when_bottomed_out = both;
                }
            },
            window,
            cx,
        );
    }

    fn slot_button(
        &self,
        title: &'static str,
        slot: ActionSlot,
        action: Action,
        cx: &mut Context<Self>,
    ) -> Div {
        let label: SharedString = if action == Action::None {
            "Choose an action".into()
        } else {
            usage::name(action).into()
        };
        v_flex()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(cx.theme().muted_foreground)
                    .child(title),
            )
            .child(
                Button::new(SharedString::from(format!("slot-{slot:?}")))
                    .w(px(SLOT_WIDTH))
                    .outline()
                    .label(label)
                    .selected(self.slot == Some(slot))
                    .tooltip("Choose this binding, then pick its action below")
                    .on_click(cx.listener(move |this, _, _, cx| this.choose_slot(slot, cx))),
            )
    }

    fn keystroke_editor(
        &self,
        entry: &advanced::Key,
        keystroke: DynamicKeystroke,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Div {
        let muted = cx.theme().muted_foreground;
        let header = h_flex()
            .gap_2()
            .child(
                div()
                    .w(px(SLOT_WIDTH))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child("Binding"),
            )
            .children(Point::ALL.map(|point| {
                div()
                    .w(px(CELL_WIDTH))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child(point.title())
            }));
        let rows: Vec<Div> = keystroke
            .strokes
            .iter()
            .enumerate()
            .map(|(index, stroke)| {
                let slot = ActionSlot::Stroke(index);
                let label: SharedString = if stroke.action == Action::None {
                    format!("Binding {}: add an action", index + 1).into()
                } else {
                    usage::name(stroke.action).into()
                };
                let bound = stroke.action != Action::None;
                let cells: Vec<Div> = Point::ALL
                    .into_iter()
                    .map(|point| event_cell(index, point, point.event(*stroke), bound, cx))
                    .collect();
                h_flex()
                    .gap_2()
                    .child(
                        Button::new(SharedString::from(format!("slot-stroke-{index}")))
                            .w(px(SLOT_WIDTH))
                            .small()
                            .outline()
                            .label(label)
                            .selected(self.slot == Some(slot))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.choose_slot(slot, cx)),
                            ),
                    )
                    .children(cells)
            })
            .collect();
        let analog = keystroke
            .strokes
            .iter()
            .any(|stroke| kind::follows_travel(stroke.action));
        v_flex()
            .gap_2()
            .child(div().text_sm().child(
                "Assign up to four bindings to each of the four parts of a key press: the start of \
                 the press, when the key is fully pressed, when the key begins to release from \
                 being fully pressed, and when the key is fully released. Press holds a binding \
                 down until a Release; Tap triggers it once.",
            ))
            .child(header)
            .children(rows)
            .when(analog, |this| {
                this.child(panel::note(
                    "Analog gamepad bindings are always active across the entire key range.",
                    cx,
                ))
            })
            .child(self.bottom_out_row(entry, keystroke, keyboard, cx))
            .child(panel::note(
                "Tip: You can create 2-state movement by adding a game's sprint (usually shift) \
                 from the 2nd action (key fully pressed) up to the 3rd action (release of the \
                 key) in addition to the regular movement key (usually w, a, s, d).",
                cx,
            ))
    }

    fn bottom_out_row(
        &self,
        entry: &advanced::Key,
        keystroke: DynamicKeystroke,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Div {
        let key = usize::from(entry.key.get());
        let custom = keystroke.bottom_out.is_some();
        let label: SharedString = match keystroke.bottom_out {
            Some(distance) => format!(
                "Fully pressed at {:.2} mm",
                keyboard.distance_mm(key, distance)
            )
            .into(),
            None => "Fully pressed at the default point".into(),
        };
        panel::row(
            label,
            Some(
                "How far the key travels before it counts as fully pressed. The default lies \
                 close to the bottom of the switch."
                    .into(),
            ),
            h_flex()
                .gap_3()
                .child(
                    Switch::new("dks-custom-bottom-out")
                        .checked(custom)
                        .label("Custom")
                        .on_change(cx.listener(|this, checked: &bool, window, cx| {
                            let millimetres =
                                checked.then(|| this.bottom_out.read(cx).value().end());
                            this.set_bottom_out(millimetres, window, cx);
                        })),
                )
                .child(
                    div()
                        .w(px(SLIDER_WIDTH))
                        .child(Slider::new(&self.bottom_out).disabled(!custom)),
                ),
            cx,
        )
    }

    fn mod_tap_editor(&self, mod_tap: ModTap, cx: &mut Context<Self>) -> Div {
        let interrupts = kind::interrupts();
        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .gap_4()
                    .flex_wrap()
                    .child(self.slot_button("Hold", ActionSlot::Hold, mod_tap.hold, cx))
                    .child(self.slot_button("Tap", ActionSlot::Tap, mod_tap.tap, cx)),
            )
            .child(panel::row(
                SharedString::from(format!("Hold Duration · {} ms", mod_tap.hold_time_ms)),
                Some(
                    "Set how long you need to hold the key before its \"Hold\" key activates."
                        .into(),
                ),
                div()
                    .w(px(SLIDER_WIDTH))
                    .child(Slider::new(&self.hold_time)),
                cx,
            ))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Just-in-time behaviour"),
                    )
                    .child(panel::note(
                        "Decides which function to trigger when you press another key before the \
                         Mod Tap activates. Fast typing can cause missed modifiers, e.g. Shift not \
                         capitalising a letter because you pressed the next key too quickly. JIT \
                         fixes this by making an immediate decision, ensuring your modifiers \
                         activate when you need them.",
                        cx,
                    ))
                    .child(choices(
                        "mod-tap-jit",
                        interrupts,
                        mod_tap.interrupt,
                        Self::set_interrupt,
                        cx,
                    )),
            )
    }

    fn toggle_editor(&self, action: Action, cx: &mut Context<Self>) -> Div {
        v_flex()
            .gap_3()
            .child(self.slot_button("Toggled action", ActionSlot::Toggle, action, cx))
            .child(panel::note(
                "Locking an action is equal to holding down a key. Normal key behaviour is equal \
                 to holding down and then releasing a key.",
                cx,
            ))
            .child(panel::note(
                "Tip: Use this with a game's crouch binding (usually C) to have both hold to \
                 crouch and toggle crouch on a single key.",
                cx,
            ))
    }

    fn depth_editor(
        first: &str,
        second: &str,
        both_bottomed_out: Resolution,
        cx: &mut Context<Self>,
    ) -> Div {
        v_flex()
            .gap_3()
            .child(pair_keys(first, second, cx))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("When both keys are fully pressed"),
            )
            .child(choices(
                "depth-bottomed-out",
                kind::bottomed_out_resolutions(first, second),
                both_bottomed_out,
                Self::set_resolution,
                cx,
            ))
            .child(panel::note(
                "You can customise it by adjusting the Actuation Point and Rapid Trigger settings \
                 of both keys on the Performance page.",
                cx,
            ))
    }

    fn socd_editor(first: &str, second: &str, pair: Socd, cx: &mut Context<Self>) -> Div {
        v_flex()
            .gap_3()
            .child(pair_keys(first, second, cx))
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("SOCD Cleaning behaviour"),
                    )
                    .child(panel::note("Select what happens when both keys are activated.", cx)),
            )
            .child(choices(
                "socd-resolution",
                kind::socd_resolutions(first, second),
                pair.resolution,
                Self::set_resolution,
                cx,
            ))
            .child(panel::row(
                "Alternative fully pressed behaviour",
                Some(
                    "Fully pressing both keys at the same time registers your inputs as normal for \
                     a more natural feel."
                        .into(),
                ),
                Switch::new("socd-both-bottomed-out")
                    .checked(pair.both_when_bottomed_out)
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        this.set_both_when_bottomed_out(*checked, window, cx);
                    })),
                cx,
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().warning)
                    .child(
                        "Use of Snappy Tappy (SOCD) is prohibited in certain games, such as \
                         Counter-Strike 2. We advise caution when using SOCD in competitive \
                         environments as it may lead to penalties.",
                    ),
            )
    }
}

fn pair_keys(first: &str, second: &str, cx: &mut Context<AdvancedPage>) -> Div {
    let muted = cx.theme().muted_foreground;
    let key = |title: &'static str, name: &str| {
        v_flex()
            .gap_0p5()
            .child(div().text_xs().text_color(muted).child(title))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(SharedString::from(name.to_owned())),
            )
    };
    h_flex()
        .gap_6()
        .child(key("1st Key", first))
        .child(key("2nd Key", second))
}

fn choices<T: Copy + PartialEq + std::fmt::Debug + 'static, const COUNT: usize>(
    id: &'static str,
    choices: [Choice<T>; COUNT],
    current: T,
    apply: fn(&mut AdvancedPage, T, &mut Window, &mut Context<AdvancedPage>),
    cx: &mut Context<AdvancedPage>,
) -> Div {
    let theme = cx.theme();
    let (primary, border, muted, radius) = (
        theme.primary,
        theme.border,
        theme.muted_foreground,
        theme.radius,
    );
    let rows = choices.map(|choice| {
        let selected = choice.value == current;
        let value = choice.value;
        h_flex()
            .id(SharedString::from(format!("{id}-{value:?}")))
            .gap_3()
            .items_start()
            .px_3()
            .py_2()
            .rounded(radius)
            .border_1()
            .border_color(if selected { primary } else { border })
            .cursor_pointer()
            .child(
                Icon::new(if selected {
                    Catalog::CircleCheck
                } else {
                    Catalog::Circle
                })
                .small(),
            )
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(choice.title),
                    )
                    .child(div().text_xs().text_color(muted).child(choice.description)),
            )
            .on_click(cx.listener(move |this, _, window, cx| apply(this, value, window, cx)))
    });
    v_flex().gap_1().children(rows)
}

fn event_cell(
    stroke: usize,
    point: Point,
    event: StrokeEvent,
    bound: bool,
    cx: &mut Context<AdvancedPage>,
) -> Div {
    let page = cx.entity().downgrade();
    let trigger = Button::new(SharedString::from(format!("dks-{stroke}-{point:?}")))
        .w(px(CELL_WIDTH))
        .small()
        .outline()
        .label(kind::event_label(event))
        .selected(event != StrokeEvent::Nothing)
        .tooltip(if bound {
            kind::event_description(event)
        } else {
            "Add an action to this binding first"
        })
        .disabled(!bound);
    div().child(trigger.dropdown_menu(move |menu, _, _| {
        STROKE_EVENTS.into_iter().fold(menu, |menu, choice| {
            let page = page.clone();
            menu.item(
                PopupMenuItem::new(kind::event_description(choice))
                    .checked(choice == event)
                    .on_click(move |_, window, cx| {
                        page.update(cx, |page, cx| {
                            page.set_stroke_event(stroke, point, choice, window, cx);
                        })
                        .ok();
                    }),
            )
        })
    }))
}
