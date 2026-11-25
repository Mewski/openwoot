use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, WeakEntity, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::{Input, InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{lighting::Rgb, profile};

use super::{ProfilesPage, Subject, icon, profile_name};
use crate::{
    color_picker::{self, ColorPicker, ColorPickerEvent, hsla_of},
    panel,
};

const DIALOG_WIDTH: f32 = 600.0;
const BANNER_HEIGHT: f32 = 64.0;
const AVATAR_SIZE: f32 = 80.0;
const AVATAR_LEFT: f32 = 40.0;
const AVATAR_TOP: f32 = 24.0;
pub(super) const BODY_TOP: f32 = AVATAR_TOP + AVATAR_SIZE - BANNER_HEIGHT + 24.0;

pub(super) struct Editor {
    page: WeakEntity<ProfilesPage>,
    subject: Subject,
    name: Entity<InputState>,
    picker: Entity<ColorPicker>,
    color: Rgb,
    icon: u8,
    problem: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl Editor {
    pub(super) fn open(
        page: WeakEntity<ProfilesPage>,
        subject: Subject,
        info: &profile::Info,
        window: &mut Window,
        cx: &mut App,
    ) {
        let editor = cx.new(|cx| Self::new(page, subject, info, window, cx));
        window.open_dialog(cx, move |dialog, _, cx| {
            let confirmed = editor.clone();
            let saved = editor.clone();
            dialog
                .w(px(DIALOG_WIDTH))
                .p_0()
                .close_button(false)
                .on_ok(move |_, window, cx| {
                    confirmed.update(cx, |editor, cx| editor.save(window, cx))
                })
                .child(editor.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .px_6()
                        .py_4()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            Button::new("edit-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("edit-save")
                                .primary()
                                .label("Save changes")
                                .on_click(move |_, window, cx| {
                                    if saved.update(cx, |editor, cx| editor.save(window, cx)) {
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                )
        });
    }

    fn new(
        page: WeakEntity<ProfilesPage>,
        subject: Subject,
        info: &profile::Info,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Profile name")
                .default_value(info.name.as_str().to_owned())
        });
        let picker = cx.new(|cx| ColorPicker::new(info.color, window, cx));
        let subscriptions = vec![
            cx.subscribe_in(
                &picker,
                window,
                |this, _, event: &ColorPickerEvent, _, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        this.color = *color;
                        cx.notify();
                    }
                },
            ),
            cx.subscribe_in(&name, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.problem = None;
                    cx.notify();
                }
            }),
        ];
        Self {
            page,
            subject,
            name,
            picker,
            color: info.color,
            icon: info.icon,
            problem: None,
            _subscriptions: subscriptions,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let text = self.name.read(cx).value();
        let name = match profile_name(&text) {
            Ok(name) => name,
            Err(problem) => {
                self.problem = Some(problem.into());
                cx.notify();
                return false;
            }
        };
        let info = profile::Info {
            name,
            color: self.color,
            icon: self.icon,
        };
        let subject = self.subject.clone();
        ColorPicker::remember(self.color, cx);
        self.page
            .update(cx, |page, cx| page.apply_edit(subject, info, window, cx))
            .ok();
        true
    }
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let editor = cx.entity().downgrade();
        let pick = move |icon: u8, _: &mut Window, cx: &mut App| {
            editor
                .update(cx, |editor, cx| {
                    editor.icon = icon;
                    cx.notify();
                })
                .ok();
        };
        v_flex()
            .w_full()
            .child(banner(
                "edit-profile-color",
                &self.picker,
                self.color,
                self.icon,
                pick,
                cx,
            ))
            .child(
                v_flex()
                    .w_full()
                    .px_10()
                    .pt(px(BODY_TOP))
                    .pb_6()
                    .gap_2()
                    .child(panel::field("Profile name", Input::new(&self.name), cx))
                    .children(
                        self.problem
                            .clone()
                            .map(|problem| div().text_xs().text_color(theme.danger).child(problem)),
                    ),
            )
    }
}

pub(super) fn banner(
    id: &'static str,
    picker: &Entity<ColorPicker>,
    color: Rgb,
    icon: u8,
    on_pick: impl Fn(u8, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let text = banner_text(color, cx);
    let change =
        color_picker::popover(id, picker).trigger(move |open, cx| change_face(text, open, cx));
    div()
        .relative()
        .w_full()
        .h(px(BANNER_HEIGHT))
        .rounded_t(theme.radius_lg)
        .bg(hsla_of(color))
        .flex()
        .items_center()
        .justify_center()
        .child(div().flex_none().child(change))
        .child(
            div()
                .absolute()
                .left(px(AVATAR_LEFT))
                .top(px(AVATAR_TOP))
                .child(icon::picker(
                    (id, 1_usize),
                    hsla_of(color),
                    icon,
                    px(AVATAR_SIZE),
                    on_pick,
                )),
        )
}

fn banner_text(color: Rgb, cx: &App) -> Hsla {
    let luminance = 0.2126 * f32::from(color.red)
        + 0.7152 * f32::from(color.green)
        + 0.0722 * f32::from(color.blue);
    if luminance > 150.0 {
        cx.theme().background
    } else {
        cx.theme().foreground
    }
}

fn change_face(text: Hsla, open: bool, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let hover = text.opacity(0.16);
    h_flex()
        .h(px(32.0))
        .px_3()
        .gap_2()
        .rounded(theme.radius)
        .text_color(text)
        .when(open, |this| this.bg(hover))
        .hover(move |style| style.bg(hover))
        .child(Icon::new(Catalog::PaintBucket).small())
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child("Change colour"),
        )
        .into_any_element()
}
