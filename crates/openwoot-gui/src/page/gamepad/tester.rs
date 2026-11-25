use gpui_kit::{
    App, Context, Div, Hsla, InteractiveElement as _, ParentElement as _, PathBuilder,
    SharedString, StatefulInteractiveElement as _, Styled as _, canvas,
    component::{ActiveTheme as _, h_flex, tooltip::Tooltip, v_flex},
    div, fill, point,
    prelude::FluentBuilder as _,
    px, relative, size,
};
use openwoot_protocol::gamepad::{self, Control, Normalization, Player};

use super::{
    GamepadPage,
    controls::{self, Controller},
    glyph::{self, Ink},
    output::{self, Axis, Stick},
};
use crate::{panel, store::Keyboard, switch::Switch};

const DISC: f32 = 140.0;
const DISC_ICON: f32 = 34.0;
const AXIS_ICON: f32 = 30.0;
const BUTTON_ICON: f32 = 34.0;
const BAR_HEIGHT: f32 = 12.0;
const VALUE_WIDTH: f32 = 48.0;
const SQUARE_RADIUS: f32 = 10.0;
const SLOT: f32 = 15.0;

#[derive(Clone, Copy)]
struct Tones {
    rim: Hsla,
    line: Hsla,
    vector: Hsla,
    track: Hsla,
    fill: Hsla,
    text: Hsla,
}

#[derive(Clone, Copy)]
struct SlotColors {
    first: Hsla,
    other: Hsla,
    empty: Hsla,
    text: Hsla,
}

impl GamepadPage {
    pub(super) fn tester_card(&mut self, keyboard: &Keyboard, cx: &mut Context<Self>) -> Div {
        let card = panel::section(
            "Test gamepad",
            Some("Press mapped keys to see the controller the computer gets.".into()),
            cx,
        );
        let Some(controller) = self.controller() else {
            return card.child(panel::note("Enable the gamepad to use the tester.", cx));
        };
        let output = self
            .controller_output(keyboard)
            .unwrap_or_else(|| output::rest(controller));
        let square = self
            .analog
            .is_some_and(|settings| settings.normalization == Normalization::Square);
        let ink = Ink::of(cx);
        let theme = cx.theme();
        let tones = Tones {
            rim: theme.border,
            line: theme.border,
            vector: theme.primary,
            track: theme.background,
            fill: theme.primary,
            text: theme.muted_foreground,
        };
        let slot_colors = SlotColors {
            first: theme.success,
            other: theme.warning,
            empty: theme.border,
            text: theme.muted_foreground,
        };
        let [first, second] = output.sticks;
        let sticks = h_flex()
            .flex_none()
            .gap_6()
            .child(disc(
                first,
                glyph::icon(Control::LeftStickPress, controller, ink, px(DISC_ICON)),
                square,
                tones,
            ))
            .child(disc(
                second,
                glyph::icon(Control::RightStickPress, controller, ink, px(DISC_ICON)),
                square,
                tones,
            ));
        let axes = v_flex().flex_1().min_w_0().gap_3().children(
            output
                .axes
                .iter()
                .map(|axis| bar(*axis, controller, ink, tones)),
        );
        let buttons = h_flex()
            .min_h(px(BUTTON_ICON))
            .flex_wrap()
            .gap_2()
            .children(
                output
                    .pressed
                    .iter()
                    .map(|control| glyph::icon(*control, controller, ink, px(BUTTON_ICON))),
            );
        card.when(
            cfg!(target_os = "macos") && controller == Controller::Xbox,
            |this| {
                this.child(panel::note(
                    "macOS has no Xbox controller driver; switch to the Classic controller.",
                    cx,
                ))
            },
        )
        .when_some(
            self.gamepad_state
                .filter(|_| controller == Controller::Xbox),
            |this, state| this.child(slot_row(state, slot_colors, cx)),
        )
        .child(
            h_flex()
                .w_full()
                .items_start()
                .gap_8()
                .child(sticks)
                .child(axes),
        )
        .child(panel::field("Buttons", buttons, cx))
        .child(panel::toggle(
            "Identify controller",
            Some("Holds the sticks still so you can spot the keyboard among controllers.".into()),
            Switch::new("gamepad-detection")
                .checked(self.detecting)
                .on_change(cx.listener(|this, checked: &bool, _, cx| {
                    this.set_detection(*checked, cx);
                })),
            cx,
        ))
    }
}

fn slot_row(state: gamepad::State, colors: SlotColors, cx: &App) -> Div {
    let help = match state.player {
        Some(Player::One) => "The computer uses the keyboard as player 1.",
        Some(_) => "Some games only read player 1; connect the keyboard before other controllers.",
        None => "The computer has not given the keyboard a player yet.",
    };
    let status = state.player.map_or_else(
        || "Not assigned".to_owned(),
        |player| format!("Player {}", player.number()),
    );
    let squares = h_flex()
        .gap_1p5()
        .items_center()
        .children(Player::ALL.map(|slot| {
            let active = state.player == Some(slot);
            let color = if slot == Player::One {
                colors.first
            } else {
                colors.other
            };
            div()
                .size(px(SLOT))
                .rounded(px(3.0))
                .border_1()
                .border_color(if active { color } else { colors.empty })
                .when(active, |this| this.bg(color))
        }))
        .child(div().ml_2().text_sm().text_color(colors.text).child(status));
    panel::row("Active slot", Some(help.into()), squares, cx)
}

fn disc(stick: Stick, icon: Div, square: bool, tones: Tones) -> Div {
    let Stick { x, y } = stick;
    let radius = if square { SQUARE_RADIUS } else { DISC / 2.0 };
    div()
        .relative()
        .flex_none()
        .size(px(DISC))
        .rounded(px(radius))
        .border_1()
        .border_color(tones.rim)
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    let centre = bounds.center();
                    window.paint_quad(fill(
                        gpui_kit::Bounds {
                            origin: point(bounds.origin.x, centre.y),
                            size: size(bounds.size.width, px(1.0)),
                        },
                        tones.line,
                    ));
                    window.paint_quad(fill(
                        gpui_kit::Bounds {
                            origin: point(centre.x, bounds.origin.y),
                            size: size(px(1.0), bounds.size.height),
                        },
                        tones.line,
                    ));
                    let tip = point(
                        centre.x + bounds.size.width * (x.clamp(-1.0, 1.0) / 2.0),
                        centre.y + bounds.size.height * (y.clamp(-1.0, 1.0) / 2.0),
                    );
                    let mut vector = PathBuilder::stroke(px(2.0));
                    vector.move_to(centre);
                    vector.line_to(tip);
                    if let Ok(path) = vector.build() {
                        window.paint_path(path, tones.vector);
                    }
                },
            )
            .absolute()
            .size_full(),
        )
        .child(
            div()
                .absolute()
                .left(relative(f32::midpoint(x.clamp(-1.0, 1.0), 1.0)))
                .top(relative(f32::midpoint(y.clamp(-1.0, 1.0), 1.0)))
                .ml(px(-DISC_ICON / 2.0))
                .mt(px(-DISC_ICON / 2.0))
                .child(icon),
        )
}

fn bar(axis: Axis, controller: Controller, ink: Ink, tones: Tones) -> Div {
    let value = axis.value.clamp(-1.0, 1.0);
    let (start, width) = if axis.centred {
        let half = value.abs() / 2.0;
        (if value < 0.0 { 0.5 - half } else { 0.5 }, half)
    } else {
        (0.0, value.max(0.0))
    };
    let name: SharedString = controls::name(axis.control, controller).into();
    h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .child(
            div()
                .id(SharedString::from(format!(
                    "tester-axis-{:?}",
                    axis.control
                )))
                .flex_none()
                .child(glyph::icon(axis.control, controller, ink, px(AXIS_ICON)))
                .tooltip(move |window, cx| Tooltip::new(name.clone()).build(window, cx)),
        )
        .child(
            div()
                .relative()
                .flex_1()
                .h(px(BAR_HEIGHT))
                .rounded_full()
                .bg(tones.track)
                .border_1()
                .border_color(tones.rim)
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(start))
                        .w(relative(width))
                        .bg(tones.fill),
                )
                .when(axis.centred, |this| {
                    this.child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(relative(0.5))
                            .w(px(1.0))
                            .bg(tones.line),
                    )
                }),
        )
        .child(
            div()
                .w(px(VALUE_WIDTH))
                .flex_none()
                .text_right()
                .text_sm()
                .text_color(tones.text)
                .child(format!("{:.0}%", value * 100.0)),
        )
}
