use std::time::Duration;

use gpui_kit::{
    Context, Div, InteractiveElement as _, KeyDownEvent, KeyUpEvent, Modifiers,
    ModifiersChangedEvent, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, Task, Window,
    component::{
        ActiveTheme as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex, v_flex,
    },
    div, px, relative,
};
use openwoot_protocol::{
    advanced,
    endpoints::GetSensorValues,
    keys::Action,
    sensors::{self, Reading},
};

use super::{AdvancedPage, rules};
use crate::{page::remap::usage_of_key, panel, usage};

const POLL_INTERVAL: Duration = Duration::from_millis(33);
const VISIBLE_WITHIN: Duration = Duration::from_millis(500);
const FULL_TRAVEL: f32 = 4095.0;
const BAR_WIDTH: f32 = 160.0;
const VALUE_WIDTH: f32 = 64.0;

pub(super) struct Tester {
    travel: Option<sensors::Values>,
    held: Vec<SharedString>,
    modifiers: Modifiers,
    _poll: Task<()>,
}

impl AdvancedPage {
    pub(super) fn start_test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let device = self.store.read(cx).device();
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let Ok(visible) = this.update(cx, |page, _| page.seen.elapsed() < VISIBLE_WITHIN)
                else {
                    return;
                };
                if !visible {
                    continue;
                }
                let reading = device
                    .configure::<GetSensorValues, sensors::Values>(Reading::Travel)
                    .await
                    .ok();
                let updated = this.update(cx, |page, cx| {
                    if let Some(tester) = &mut page.tester {
                        tester.travel = reading;
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    return;
                }
            }
        });
        self.tester = Some(Tester {
            travel: None,
            held: Vec::new(),
            modifiers: Modifiers::default(),
            _poll: poll,
        });
        self.tester_focus.focus(window, cx);
        cx.notify();
    }

    pub(super) fn stop_test(&mut self, cx: &mut Context<Self>) {
        if self.tester.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn tested_travel(&self, key: usize) -> Option<f32> {
        let tester = self.tester.as_ref()?;
        let entry = self.selected_entry()?;
        if !rules::bindings(entry).any(|(bound, _)| usize::from(bound.get()) == key) {
            return None;
        }
        let travel = tester.travel.as_ref()?.get(key).copied().flatten()?;
        Some((f32::from(travel) / FULL_TRAVEL).clamp(0.0, 1.0))
    }

    pub(super) fn tester_rows(&self, entry: &advanced::Key, cx: &mut Context<Self>) -> Div {
        let testing = self.tester.is_some();
        let toggle = Button::new("advanced-test")
            .ghost()
            .small()
            .label(if testing { "Stop" } else { "Start" })
            .on_click(cx.listener(|this, _, window, cx| {
                if this.tester.is_some() {
                    this.stop_test(cx);
                } else {
                    this.start_test(window, cx);
                }
            }));
        let row = panel::row(
            "Test",
            Some("Shows key travel and what the computer receives".into()),
            toggle,
            cx,
        );
        let Some(tester) = &self.tester else {
            return row;
        };
        let bars: Vec<Div> = rules::bindings(entry)
            .map(|(key, _)| {
                let key = usize::from(key.get());
                panel::row(
                    SharedString::from(self.key_name(key)),
                    None,
                    travel_bar(tester, key, cx),
                    cx,
                )
            })
            .collect();
        let mut received = modifier_names(tester.modifiers);
        received.extend(tester.held.iter().cloned());
        let received: SharedString = if received.is_empty() {
            "Nothing".into()
        } else {
            received.join(" + ").into()
        };
        div().child(
            v_flex()
                .id("advanced-tester")
                .track_focus(&self.tester_focus)
                .on_key_down(cx.listener(Self::tested_key_down))
                .on_key_up(cx.listener(Self::tested_key_up))
                .on_modifiers_changed(cx.listener(Self::tested_modifiers))
                .on_click(cx.listener(|this, _, window, cx| this.tester_focus.focus(window, cx)))
                .gap_1()
                .child(row)
                .children(bars)
                .child(panel::row(
                    "Received",
                    None,
                    div().text_sm().child(received),
                    cx,
                )),
        )
    }

    fn tested_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(tester) = &mut self.tester else {
            return;
        };
        cx.stop_propagation();
        let name = received_name(&event.keystroke.key);
        if !tester.held.contains(&name) {
            tester.held.push(name);
            cx.notify();
        }
    }

    fn tested_key_up(&mut self, event: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(tester) = &mut self.tester else {
            return;
        };
        cx.stop_propagation();
        let name = received_name(&event.keystroke.key);
        tester.held.retain(|held| *held != name);
        cx.notify();
    }

    fn tested_modifiers(
        &mut self,
        event: &ModifiersChangedEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(tester) = &mut self.tester {
            tester.modifiers = event.modifiers;
            cx.notify();
        }
    }
}

fn travel_bar(tester: &Tester, key: usize, cx: &Context<AdvancedPage>) -> Div {
    let theme = cx.theme();
    let fraction = tester
        .travel
        .as_ref()
        .and_then(|travel| travel.get(key).copied().flatten())
        .map_or(0.0, |travel| {
            (f32::from(travel) / FULL_TRAVEL).clamp(0.0, 1.0)
        });
    h_flex()
        .gap_3()
        .items_center()
        .child(
            div()
                .w(px(BAR_WIDTH))
                .h_1p5()
                .rounded(theme.radius)
                .bg(theme.secondary)
                .child(
                    div()
                        .h_full()
                        .w(relative(fraction))
                        .rounded(theme.radius)
                        .bg(theme.primary),
                ),
        )
        .child(
            div()
                .w(px(VALUE_WIDTH))
                .text_right()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(SharedString::from(format!("{:.0} %", fraction * 100.0))),
        )
}

fn received_name(key: &str) -> SharedString {
    usage_of_key(key).map_or_else(
        || SharedString::from(key.to_owned()),
        |code| usage::legend(Action::Keyboard(code)).into(),
    )
}

fn modifier_names(modifiers: Modifiers) -> Vec<SharedString> {
    [
        (modifiers.control, "Ctrl"),
        (modifiers.shift, "Shift"),
        (modifiers.alt, "Alt"),
        (modifiers.platform, "Win/Cmd"),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .map(|(_, name)| SharedString::from(name))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn received_keys_are_named_like_their_legends() {
        assert_eq!(received_name("a"), SharedString::from("A"));
        assert_eq!(received_name("left"), SharedString::from("←"));
        assert_eq!(received_name("volume"), SharedString::from("volume"));
    }

    #[test]
    fn held_modifiers_are_listed_in_a_fixed_order() {
        let modifiers = Modifiers {
            shift: true,
            control: true,
            ..Modifiers::default()
        };
        assert_eq!(
            modifier_names(modifiers),
            vec![SharedString::from("Ctrl"), SharedString::from("Shift")]
        );
    }
}
