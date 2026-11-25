use gpui_kit::{
    AnyElement, App, AppContext as _, BoxShadow, Context, Div, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, WeakEntity, Window,
    assets::IconName as Catalog,
    black,
    component::{
        ActiveTheme as _, Icon, WindowExt as _, h_flex,
        input::{InputEvent, InputState},
        v_flex,
    },
    div, point, px, white,
};
use openwoot_protocol::{apps::Link, lighting::Rgb, profile};

use super::{
    ProfilesPage, Subject,
    apps::{AppList, AppListEvent},
    card::tipped_beside,
    icon, profile_name,
};
use crate::{
    color_picker::{self, ColorPicker, ColorPickerEvent, hsla_of},
    panel::{self, ActionButton, Tip},
    store::Store,
};

const DIALOG_WIDTH: f32 = 820.0;
const DIALOG_SHADOWS: [(f32, f32, f32); 2] = [(20.0, 25.0, -5.0), (8.0, 10.0, -6.0)];
const RADIUS: f32 = 8.0;
const BANNER_HEIGHT: f32 = 64.0;
const BANNER_HOVER: f32 = 0.25;
const LIGHT_BANNER: f32 = 150.0;
const CHANGE_PADDING: f32 = 8.0;
const CHANGE_GAP: f32 = 4.0;
const CHANGE_ICON: f32 = 16.0;
const CHANGE_TEXT: f32 = 14.0;
const CHANGE_LINE: f32 = 16.8;
const AVATAR_SIZE: f32 = 80.0;
const AVATAR_LEFT: f32 = 40.0;
const AVATAR_TOP: f32 = 16.0;
const AVATAR_GAP: f32 = 32.0;
const COLOUR_GAP: f32 = 4.0;
const ICONS_GAP: f32 = 16.0;
const NAME_PLACEHOLDER: &str = "Profile name";
pub(super) const BODY_PADDING_X: f32 = 40.0;
pub(super) const BODY_TOP: f32 = AVATAR_TOP + AVATAR_SIZE - BANNER_HEIGHT + AVATAR_GAP;
const BODY_BOTTOM: f32 = 28.0;
const FIELD_GAP: f32 = 8.0;
const FIELD_INSET: f32 = 1.0;
const FOOTER_PADDING_X: f32 = 40.0;
const FOOTER_PADDING_Y: f32 = 16.0;
const FOOTER_GAP: f32 = 8.0;
const DIVIDER_OPACITY: f32 = 0.6;
const PROBLEM_TEXT: f32 = 14.0;
const APPS_GAP: f32 = 32.0;

pub(super) struct Editor {
    page: WeakEntity<ProfilesPage>,
    subject: Subject,
    name: Entity<InputState>,
    picker: Entity<ColorPicker>,
    color: Rgb,
    icon: u8,
    apps: Option<Entity<AppList>>,
    problem: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

pub(super) struct Linking {
    pub(super) store: Entity<Store>,
    pub(super) links: Vec<Link>,
}

impl Editor {
    pub(super) fn open(
        page: WeakEntity<ProfilesPage>,
        subject: Subject,
        info: &profile::Info,
        linking: Option<Linking>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let editor = cx.new(|cx| Self::new(page, subject, info, linking, window, cx));
        window.open_dialog(cx, move |dialog, _, cx| {
            let confirmed = editor.clone();
            let theme = cx.theme();
            dialog
                .w(px(DIALOG_WIDTH))
                .centered()
                .close_button(false)
                .p_0()
                .gap_0()
                .border_0()
                .rounded(px(RADIUS))
                .bg(theme.group_box)
                .shadow(dialog_shadow(theme.sidebar))
                .on_ok(move |_, window, cx| {
                    confirmed.update(cx, |editor, cx| editor.save(window, cx))
                })
                .child(editor.clone())
                .footer(footer(editor.clone(), cx))
        });
    }

    fn new(
        page: WeakEntity<ProfilesPage>,
        subject: Subject,
        info: &profile::Info,
        linking: Option<Linking>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name =
            cx.new(|cx| InputState::new(window, cx).default_value(info.name.as_str().to_owned()));
        let picker = cx.new(|cx| ColorPicker::new(info.color, window, cx));
        let linked = subject.linked_slot();
        let apps = linking.map(|linking| {
            cx.new(|cx| AppList::new(linking.store, linked, linking.links, window, cx))
        });
        let mut subscriptions = vec![
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
        subscriptions.extend(
            apps.as_ref()
                .map(|apps| cx.subscribe(apps, |_, _, _: &AppListEvent, cx| cx.notify())),
        );
        Self {
            page,
            subject,
            name,
            picker,
            color: info.color,
            icon: info.icon,
            apps,
            problem: None,
            _subscriptions: subscriptions,
        }
    }

    fn can_save(&self, cx: &App) -> bool {
        !self.subject.is_linking()
            || self
                .apps
                .as_ref()
                .is_some_and(|apps| !apps.read(cx).links().is_empty())
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.can_save(cx) {
            return false;
        }
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
        let links = self
            .apps
            .as_ref()
            .map(|apps| apps.read(cx).links().to_vec());
        let subject = self.subject.clone();
        ColorPicker::remember(self.color, cx);
        self.page
            .update(cx, |page, cx| {
                page.apply_edit(subject, info, links, window, cx);
            })
            .ok();
        true
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(header(
                "edit-profile",
                &self.picker,
                self.color,
                self.icon,
                pick,
            ))
            .child(
                v_flex()
                    .w_full()
                    .px(px(BODY_PADDING_X))
                    .pt(px(BODY_TOP))
                    .pb(px(BODY_BOTTOM))
                    .gap(px(APPS_GAP))
                    .child(
                        v_flex()
                            .w_full()
                            .gap(px(FIELD_GAP))
                            .child(panel::field(
                                "Profile Name",
                                div().p(px(FIELD_INSET)).child(panel::text_field(
                                    &self.name,
                                    NAME_PLACEHOLDER,
                                    window,
                                    cx,
                                )),
                                cx,
                            ))
                            .children(
                                self.problem
                                    .clone()
                                    .map(|problem| problem_text(problem, cx)),
                            ),
                    )
                    .children(self.apps.clone()),
            )
    }
}

pub(super) fn header(
    id: &'static str,
    picker: &Entity<ColorPicker>,
    color: Rgb,
    icon: u8,
    on_pick: impl Fn(u8, &mut Window, &mut App) + 'static,
) -> Div {
    let base = hsla_of(color);
    let text = banner_text(color);
    div()
        .relative()
        .w_full()
        .h(px(BANNER_HEIGHT))
        .child(
            color_picker::popover((id, 0_usize), picker)
                .below_centre(px(COLOUR_GAP))
                .trigger(move |_, _| banner_face(base, text)),
        )
        .child(tipped_beside(
            div()
                .id((id, 2_usize))
                .absolute()
                .left(px(AVATAR_LEFT))
                .top(px(AVATAR_TOP))
                .occlude()
                .child(
                    icon::picker((id, 1_usize), base, icon, px(AVATAR_SIZE), on_pick)
                        .right(px(ICONS_GAP)),
                ),
            (id, 3_usize),
            Tip::Text("Edit Icon".into()),
        ))
}

pub(super) fn problem_text(problem: SharedString, cx: &App) -> Div {
    div()
        .text_size(px(PROBLEM_TEXT))
        .text_color(cx.theme().danger)
        .child(problem)
}

fn footer(editor: Entity<Editor>, cx: &App) -> Div {
    let ready = editor.read(cx).can_save(cx);
    h_flex()
        .w_full()
        .justify_end()
        .gap(px(FOOTER_GAP))
        .px(px(FOOTER_PADDING_X))
        .py(px(FOOTER_PADDING_Y))
        .border_t_1()
        .border_color(cx.theme().border.opacity(DIVIDER_OPACITY))
        .child(
            ActionButton::new("edit-cancel", "Cancel")
                .large()
                .on_click(|_, window, cx| window.close_dialog(cx)),
        )
        .child(
            ActionButton::new("edit-save", "Save changes")
                .large()
                .primary()
                .disabled(!ready)
                .on_click(move |_, window, cx| {
                    if editor.update(cx, |editor, cx| editor.save(window, cx)) {
                        window.close_dialog(cx);
                    }
                }),
        )
}

fn dialog_shadow(color: Hsla) -> Vec<BoxShadow> {
    DIALOG_SHADOWS
        .into_iter()
        .map(|(offset, blur, spread)| BoxShadow {
            color,
            offset: point(px(0.0), px(offset)),
            blur_radius: px(blur),
            spread_radius: px(spread),
            inset: false,
        })
        .collect()
}

fn banner_text(color: Rgb) -> Hsla {
    let luminance = 0.2126 * f32::from(color.red)
        + 0.7152 * f32::from(color.green)
        + 0.0722 * f32::from(color.blue);
    if luminance > LIGHT_BANNER {
        black()
    } else {
        white()
    }
}

fn banner_face(base: Hsla, text: Hsla) -> AnyElement {
    let hover = base.blend(white().opacity(BANNER_HOVER));
    h_flex()
        .w_full()
        .h(px(BANNER_HEIGHT))
        .justify_center()
        .rounded_t(px(RADIUS))
        .bg(base)
        .hover(move |style| style.bg(hover))
        .child(
            h_flex()
                .p(px(CHANGE_PADDING))
                .gap(px(CHANGE_GAP))
                .text_color(text)
                .child(Icon::new(Catalog::PaintBucket).size(px(CHANGE_ICON)))
                .child(
                    div()
                        .text_size(px(CHANGE_TEXT))
                        .line_height(px(CHANGE_LINE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Change Colour"),
                ),
        )
        .into_any_element()
}
