use gpui_kit::{
    Context, Div, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _,
        button::{Button, ButtonGroup, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        slider::Slider,
        switch::Switch,
        tooltip::Tooltip,
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
    kind::{self, Kind, Point, STROKE_EVENTS},
};
use crate::{panel, store::Keyboard, usage};

const BINDING_WIDTH: f32 = 112.0;
const CELL_WIDTH: f32 = 70.0;
const VALUE_WIDTH: f32 = 64.0;

impl AdvancedPage {
    pub(super) fn editor_card(&self, keyboard: &Keyboard, cx: &mut Context<Self>) -> Option<Div> {
        let entry = *self.selected_entry()?;
        let kind = Kind::of(&entry.behaviour);
        let keys = panel::row(
            if kind.key_count() == 1 { "Key" } else { "Keys" },
            None,
            div()
                .text_sm()
                .child(SharedString::from(self.keys_name(&entry))),
            cx,
        );
        let card = panel::section(kind.title(), Some(kind.help().into()), cx).child(keys);
        let card = match entry.behaviour {
            Behaviour::DynamicKeystroke(keystroke) => card
                .child(panel::field(
                    "Bindings",
                    self.keystroke_grid(keystroke, cx),
                    cx,
                ))
                .children(self.bottom_out_rows(&entry, keystroke, keyboard, cx)),
            Behaviour::ModTap(mod_tap) => self.mod_tap_fields(card, mod_tap, cx),
            Behaviour::Toggle(action) => card.child(panel::field(
                "Action",
                self.slot_button(ActionSlot::Toggle, action, cx),
                cx,
            )),
            Behaviour::DepthPriority(pair) => {
                let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
                card.child(panel::field(
                    "Both fully pressed",
                    resolution_menu(
                        "depth-bottomed-out",
                        kind::bottomed_out_resolutions(&first, &second),
                        pair.both_bottomed_out,
                        cx,
                    ),
                    cx,
                ))
            }
            Behaviour::Socd(pair) => {
                let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
                Self::socd_fields(card, &first, &second, pair, cx)
            }
        };
        Some(
            card.child(self.tester_rows(&entry, cx)).child(
                h_flex()
                    .justify_between()
                    .child(
                        Button::new("advanced-delete")
                            .ghost()
                            .small()
                            .icon(Catalog::Trash)
                            .label("Delete")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.confirm_delete(entry, window, cx);
                            })),
                    )
                    .child(
                        Button::new("advanced-done")
                            .small()
                            .label("Done")
                            .on_click(cx.listener(|this, _, _, cx| this.close_editor(cx))),
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

    fn slot_button(&self, slot: ActionSlot, action: Action, cx: &mut Context<Self>) -> Button {
        let label: SharedString = if action == Action::None {
            "None".into()
        } else {
            usage::name(action).into()
        };
        let targeted = self.slot == Some(slot);
        let accent = cx.theme().primary;
        Button::new(SharedString::from(format!("slot-{slot:?}")))
            .w_full()
            .label(label)
            .selected(targeted)
            .when(targeted, |button| button.border_color(accent))
            .on_click(cx.listener(move |this, _, _, cx| this.choose_slot(slot, cx)))
    }

    fn keystroke_grid(&self, keystroke: DynamicKeystroke, cx: &mut Context<Self>) -> Div {
        let (muted, accent) = (cx.theme().muted_foreground, cx.theme().primary);
        let header = h_flex()
            .gap_1()
            .child(div().w(px(BINDING_WIDTH)))
            .children(Point::ALL.map(|point| {
                div()
                    .id(SharedString::from(format!("dks-point-{point:?}")))
                    .w(px(CELL_WIDTH))
                    .text_xs()
                    .text_color(muted)
                    .text_center()
                    .child(point.title())
                    .tooltip(move |window, cx| Tooltip::new(point.help()).build(window, cx))
            }));
        let rows: Vec<Div> = keystroke
            .strokes
            .iter()
            .enumerate()
            .map(|(index, stroke)| {
                let slot = ActionSlot::Stroke(index);
                let targeted = self.slot == Some(slot);
                let bound = stroke.action != Action::None;
                let label: SharedString = if bound {
                    usage::legend(stroke.action).into()
                } else {
                    "None".into()
                };
                let cells: Vec<Div> = Point::ALL
                    .into_iter()
                    .map(|point| event_cell(index, point, point.event(*stroke), bound, cx))
                    .collect();
                h_flex()
                    .gap_1()
                    .child(
                        Button::new(SharedString::from(format!("slot-stroke-{index}")))
                            .w(px(BINDING_WIDTH))
                            .outline()
                            .small()
                            .label(label)
                            .tooltip(usage::name(stroke.action))
                            .selected(targeted)
                            .when(targeted, |button| button.border_color(accent))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.choose_slot(slot, cx)),
                            ),
                    )
                    .children(cells)
            })
            .collect();
        v_flex().gap_1().child(header).children(rows)
    }

    fn bottom_out_rows(
        &self,
        entry: &advanced::Key,
        keystroke: DynamicKeystroke,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Vec<Div> {
        let key = usize::from(entry.key.get());
        let custom = keystroke.bottom_out.is_some();
        let mut rows = vec![panel::row(
            "Custom bottom-out",
            Some("Where the key counts as fully pressed".into()),
            Switch::new("dks-custom-bottom-out")
                .checked(custom)
                .on_change(cx.listener(|this, checked: &bool, window, cx| {
                    let millimetres = checked.then(|| this.bottom_out.read(cx).value().end());
                    this.set_bottom_out(millimetres, window, cx);
                })),
            cx,
        )];
        if custom {
            let millimetres = self
                .bottom_out
                .read(cx)
                .value()
                .end()
                .min(keyboard.travel_mm(key));
            rows.push(panel::field(
                "Bottom-out",
                slider_with_value(
                    Slider::new(&self.bottom_out),
                    format!("{millimetres:.2} mm"),
                    cx,
                ),
                cx,
            ));
        }
        if keystroke
            .strokes
            .iter()
            .any(|stroke| kind::follows_travel(stroke.action))
        {
            rows.push(panel::note(
                "Analog gamepad actions follow the whole key travel",
                cx,
            ));
        }
        rows
    }

    fn mod_tap_fields(&self, card: Div, mod_tap: ModTap, cx: &mut Context<Self>) -> Div {
        let milliseconds = self.hold_time.read(cx).value().end();
        let interrupts = kind::interrupts();
        let current = mod_tap.interrupt;
        let group = ButtonGroup::new("mod-tap-jit")
            .small()
            .outline()
            .children(interrupts.iter().map(|choice| {
                Button::new(SharedString::from(format!("jit-{:?}", choice.value)))
                    .label(choice.title.clone())
                    .tooltip(choice.description.clone())
                    .selected(choice.value == current)
            }))
            .on_click(cx.listener(move |this, clicks: &Vec<usize>, window, cx| {
                if let Some(choice) = clicks.first().and_then(|index| interrupts.get(*index)) {
                    this.set_interrupt(choice.value, window, cx);
                }
            }));
        let tap = panel::field(
            "Tap",
            self.slot_button(ActionSlot::Tap, mod_tap.tap, cx),
            cx,
        );
        let hold = panel::field(
            "Hold",
            self.slot_button(ActionSlot::Hold, mod_tap.hold, cx),
            cx,
        );
        card.child(
            h_flex()
                .w_full()
                .gap_4()
                .child(div().flex_1().min_w_0().child(tap))
                .child(div().flex_1().min_w_0().child(hold)),
        )
        .child(panel::field(
            "Hold time",
            slider_with_value(
                Slider::new(&self.hold_time),
                format!("{milliseconds:.0} ms"),
                cx,
            ),
            cx,
        ))
        .child(panel::field("Just-in-time", group, cx))
    }

    fn socd_fields(
        card: Div,
        first: &str,
        second: &str,
        pair: Socd,
        cx: &mut Context<Self>,
    ) -> Div {
        card.child(panel::field(
            "Behaviour",
            resolution_menu(
                "socd-resolution",
                kind::socd_resolutions(first, second),
                pair.resolution,
                cx,
            ),
            cx,
        ))
        .child(panel::row(
            "Both when fully pressed",
            Some("Fully pressing both keys registers both".into()),
            Switch::new("socd-both-bottomed-out")
                .checked(pair.both_when_bottomed_out)
                .on_change(cx.listener(|this, checked: &bool, window, cx| {
                    this.set_both_when_bottomed_out(*checked, window, cx);
                })),
            cx,
        ))
        .child(panel::note(
            "Prohibited in some games, such as Counter-Strike 2",
            cx,
        ))
    }
}

fn slider_with_value(slider: Slider, value: String, cx: &Context<AdvancedPage>) -> Div {
    h_flex()
        .w_full()
        .gap_4()
        .items_center()
        .child(div().flex_1().min_w_0().child(slider))
        .child(
            div()
                .w(px(VALUE_WIDTH))
                .text_right()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(value)),
        )
}

fn resolution_menu<const COUNT: usize>(
    id: &'static str,
    choices: [(Resolution, String); COUNT],
    current: Resolution,
    cx: &mut Context<AdvancedPage>,
) -> Div {
    let page = cx.entity().downgrade();
    let label = choices
        .iter()
        .find(|(value, _)| *value == current)
        .map_or_else(String::new, |(_, title)| title.clone());
    div().w_full().child(
        Button::new(id)
            .w_full()
            .label(label)
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                choices.iter().fold(menu, |menu, (value, title)| {
                    let page = page.clone();
                    let value = *value;
                    menu.item(
                        PopupMenuItem::new(title.clone())
                            .checked(value == current)
                            .on_click(move |_, window, cx| {
                                page.update(cx, |page, cx| {
                                    page.set_resolution(value, window, cx);
                                })
                                .ok();
                            }),
                    )
                })
            }),
    )
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
        .outline()
        .small()
        .label(kind::event_label(event))
        .selected(event != StrokeEvent::Nothing)
        .tooltip(kind::event_help(event))
        .disabled(!bound);
    div().child(trigger.dropdown_menu(move |menu, _, _| {
        STROKE_EVENTS.into_iter().fold(menu, |menu, choice| {
            let page = page.clone();
            menu.item(
                PopupMenuItem::new(kind::event_name(choice))
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
