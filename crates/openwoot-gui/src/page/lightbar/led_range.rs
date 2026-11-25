use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, MouseButton, MouseMoveEvent,
    ParentElement as _, SharedString, Stateful, Styled as _, Window,
    component::{ActiveTheme as _, h_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::lightbar::{
    Blink, Custom, Effect, Meter, Mirror, ProfileIndicator, Range, Solid, Target,
};

use super::{LightbarPage, entry::lightbar_range};
use crate::theme;

const DOT: f32 = 30.0;
const CELL_PADDING: f32 = 1.0;
const NUMBER_TEXT: f32 = 14.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Grip {
    First,
    Last,
}

impl LightbarPage {
    pub(super) fn led_range(&self, range: Range, cx: &Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let leds = self.leds(cx);
        h_flex()
            .id("led-range")
            .flex_wrap()
            .rounded_full()
            .bg(theme::palette().raised)
            .border_1()
            .border_color(theme.border)
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.grip = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.grip = None),
            )
            .children((0..leds).map(|led| {
                let inside = (range.first..=range.last).contains(&led);
                div()
                    .id(SharedString::from(format!("led-{led}")))
                    .flex_none()
                    .p(px(CELL_PADDING))
                    .cursor_pointer()
                    .child(
                        div()
                            .size(px(DOT))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .border_2()
                            .bg(theme.secondary)
                            .text_size(px(NUMBER_TEXT))
                            .line_height(px(NUMBER_TEXT))
                            .font_weight(FontWeight::BOLD)
                            .map(|this| {
                                if inside {
                                    this.border_color(theme.primary)
                                        .text_color(theme.foreground)
                                } else {
                                    this.border_color(theme.transparent)
                                        .text_color(theme.muted_foreground)
                                }
                            })
                            .child(SharedString::from((u16::from(led) + 1).to_string())),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| this.grab_led(led, window, cx)),
                    )
                    .on_mouse_move(
                        cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                            if event.pressed_button == Some(MouseButton::Left)
                                && this.grip.is_some()
                            {
                                this.drag_led(led, window, cx);
                            }
                        }),
                    )
            }))
    }

    fn grab_led(&mut self, led: u8, window: &mut Window, cx: &mut Context<Self>) {
        let Some(range) = self
            .chosen_entry()
            .and_then(|entry| lightbar_range(&entry.effect))
        else {
            return;
        };
        let grip = if led <= range.first {
            Grip::First
        } else if led >= range.last {
            Grip::Last
        } else if led - range.first <= range.last - led {
            Grip::First
        } else {
            Grip::Last
        };
        self.grip = Some(grip);
        self.drag_led(led, window, cx);
    }

    fn drag_led(&mut self, led: u8, window: &mut Window, cx: &mut Context<Self>) {
        let Some(grip) = self.grip else {
            return;
        };
        self.edit_range(
            |range| match grip {
                Grip::First => range.first = led.min(range.last),
                Grip::Last => range.last = led.max(range.first),
            },
            window,
            cx,
        );
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
