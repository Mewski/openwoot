use std::time::Duration;

use gpui_kit::{
    AnyElement, Context, Div, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    KeyUpEvent, Modifiers, ModifiersChangedEvent, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, Window,
    component::{ActiveTheme as _, h_flex, v_flex},
    div, px, relative,
};
use openwoot_protocol::{
    advanced,
    endpoints::GetSensorValues,
    keys::Action,
    sensors::{self, Reading},
};

use super::{AdvancedPage, editor::heading, rules};
use crate::{
    page::remap::{text_chip, usage_of_key},
    panel, usage,
};

const POLL_INTERVAL: Duration = Duration::from_millis(33);
const VISIBLE_WITHIN: Duration = Duration::from_millis(500);
const FULL_TRAVEL: f32 = 4095.0;
const BAR_WIDTH: f32 = 160.0;
const VALUE_WIDTH: f32 = 56.0;
const TITLE_GAP: f32 = 8.0;
const SECTION_GAP: f32 = 16.0;
const PANES_HEIGHT: f32 = 200.0;
const PANES_RADIUS: f32 = 8.0;
const PANES_RULE: f32 = 1.0;
const PANE_PADDING_X: f32 = 8.0;
const PANE_PADDING_Y: f32 = 12.0;
const PANE_GAP: f32 = 8.0;
const PANE_TITLE_TEXT: f32 = 12.8;
const PANE_TITLE_LINE: f32 = 19.2;
const KEYS_GAP: f32 = 4.0;
const RELEASED_KEPT: usize = 12;
const TEST_TITLE: &str = "Test your binding";

pub(super) struct Tester {
    travel: Option<sensors::Values>,
    pressed: Vec<SharedString>,
    released: Vec<SharedString>,
    modifiers: Modifiers,
    _poll: Task<()>,
}

impl AdvancedPage {
    pub(super) fn start_test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tester.is_none() {
            let device = self.store.read(cx).device();
            let poll = cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(POLL_INTERVAL).await;
                    let Ok(visible) =
                        this.update(cx, |page, _| page.seen.elapsed() < VISIBLE_WITHIN)
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
                pressed: Vec::new(),
                released: Vec::new(),
                modifiers: Modifiers::default(),
                _poll: poll,
            });
        }
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
        let entry = self.edited()?;
        if !rules::bindings(&entry).any(|(bound, _)| usize::from(bound.get()) == key) {
            return None;
        }
        let travel = tester.travel.as_ref()?.get(key).copied().flatten()?;
        Some((f32::from(travel) / FULL_TRAVEL).clamp(0.0, 1.0))
    }

    pub(super) fn tester_panes(&self, entry: &advanced::Key, cx: &mut Context<Self>) -> AnyElement {
        let Some(tester) = &self.tester else {
            return div().into_any_element();
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
        let mut pressed = modifier_names(tester.modifiers);
        pressed.extend(tester.pressed.iter().cloned());
        let theme = cx.theme();
        let (frame, surface) = (theme.secondary, theme.background);
        v_flex()
            .id("advanced-tester")
            .track_focus(&self.tester_focus)
            .on_key_down(cx.listener(Self::tested_key_down))
            .on_key_up(cx.listener(Self::tested_key_up))
            .on_modifiers_changed(cx.listener(Self::tested_modifiers))
            .on_click(cx.listener(|this, _, window, cx| this.tester_focus.focus(window, cx)))
            .gap(px(SECTION_GAP))
            .child(
                v_flex()
                    .gap(px(TITLE_GAP))
                    .child(heading(TEST_TITLE, cx))
                    .child(
                        h_flex()
                            .w_full()
                            .h(px(PANES_HEIGHT))
                            .p(px(PANES_RULE))
                            .gap(px(PANES_RULE))
                            .rounded(px(PANES_RADIUS))
                            .bg(frame)
                            .child(
                                pane("Pressed Keys", &pressed, surface, cx)
                                    .rounded_l(px(PANES_RADIUS - PANES_RULE)),
                            )
                            .child(
                                pane("Released Keys", &tester.released, surface, cx)
                                    .rounded_r(px(PANES_RADIUS - PANES_RULE)),
                            ),
                    ),
            )
            .child(v_flex().gap_1().children(bars))
            .into_any_element()
    }

    fn tested_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(tester) = &mut self.tester else {
            return;
        };
        cx.stop_propagation();
        let name = received_name(&event.keystroke.key);
        if !tester.pressed.contains(&name) {
            tester.pressed.push(name);
            cx.notify();
        }
    }

    fn tested_key_up(&mut self, event: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(tester) = &mut self.tester else {
            return;
        };
        cx.stop_propagation();
        let name = received_name(&event.keystroke.key);
        tester.pressed.retain(|held| *held != name);
        release(tester, name);
        cx.notify();
    }

    fn tested_modifiers(
        &mut self,
        event: &ModifiersChangedEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(tester) = &mut self.tester {
            let before = modifier_names(tester.modifiers);
            let after = modifier_names(event.modifiers);
            for name in before {
                if !after.contains(&name) {
                    release(tester, name);
                }
            }
            tester.modifiers = event.modifiers;
            cx.notify();
        }
    }
}

fn release(tester: &mut Tester, name: SharedString) {
    tester.released.retain(|released| *released != name);
    tester.released.insert(0, name);
    tester.released.truncate(RELEASED_KEPT);
}

fn pane(
    title: &'static str,
    keys: &[SharedString],
    surface: gpui_kit::Hsla,
    cx: &Context<AdvancedPage>,
) -> Div {
    v_flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .px(px(PANE_PADDING_X))
        .py(px(PANE_PADDING_Y))
        .gap(px(PANE_GAP))
        .bg(surface)
        .child(
            div()
                .text_size(px(PANE_TITLE_TEXT))
                .line_height(px(PANE_TITLE_LINE))
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(title.to_uppercase())),
        )
        .child(
            h_flex()
                .flex_wrap()
                .gap(px(KEYS_GAP))
                .children(keys.iter().map(|key| text_chip(key.clone(), cx))),
        )
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
