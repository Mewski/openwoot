use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, Context, Div, ElementId, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, Sizable as _, h_flex, tooltip::Tooltip, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{lighting::Rgb, profile::Slot};

use super::{ProfilesPage, icon::avatar};
use crate::{
    color_picker::hsla_of,
    dropdown::{Dropdown, Item},
};

const CARD_HEIGHT: f32 = 72.0;
const CARD_AVATAR: f32 = 44.0;
const PREVIEW_WIDTH: f32 = 300.0;
const PREVIEW_HEIGHT: f32 = 60.0;
const PREVIEW_AVATAR: f32 = 34.0;
const MENU_WIDTH: f32 = 220.0;
const MENU_BUTTON: f32 = 28.0;
const CHIP_HEIGHT: f32 = 20.0;
const TOOLTIP_WIDTH: f32 = 360.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Origin {
    Onboard(Slot),
    Inactive(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Place {
    Onboard(Slot),
    NewOnboard,
    Inactive,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Moving {
    pub(super) origin: Origin,
    name: SharedString,
    color: Rgb,
    icon: u8,
}

pub(super) struct Card {
    pub(super) origin: Origin,
    pub(super) name: SharedString,
    pub(super) color: Rgb,
    pub(super) icon: u8,
    pub(super) number: Option<u8>,
    pub(super) active: bool,
    pub(super) default: bool,
    pub(super) shortcut: Vec<SharedString>,
    pub(super) menu: Vec<(Item, super::Command)>,
    pub(super) busy: bool,
}

impl Card {
    pub(super) fn render(self, cx: &Context<ProfilesPage>) -> Stateful<Div> {
        let theme = cx.theme();
        let origin = self.origin;
        let id: ElementId = match origin {
            Origin::Onboard(slot) => ("onboard-profile", usize::from(slot.get())).into(),
            Origin::Inactive(index) => ("inactive-profile", index).into(),
        };
        let place = match origin {
            Origin::Onboard(slot) => Place::Onboard(slot),
            Origin::Inactive(_) => Place::Inactive,
        };
        let moving = Moving {
            origin,
            name: self.name.clone(),
            color: self.color,
            icon: self.icon,
        };
        let border = if self.active {
            theme.primary
        } else {
            theme.secondary
        };
        let hover = theme.secondary_hover;
        let primary = theme.primary;
        h_flex()
            .id(id)
            .relative()
            .w_full()
            .h(px(CARD_HEIGHT))
            .pl_5()
            .pr_2()
            .gap_3()
            .rounded(theme.radius_lg)
            .bg(theme.secondary)
            .border_1()
            .border_color(border)
            .when(!self.active, |this| {
                this.hover(move |style| style.border_color(hover))
            })
            .when(!self.busy, |this| {
                this.on_drag(moving, |moving, _, _, cx| cx.new(|_| moving.clone()))
                    .drag_over::<Moving>(move |style, _, _, _| style.border_color(primary))
                    .on_drop(cx.listener(move |page, moving: &Moving, window, cx| {
                        page.drop_profile(moving, place, window, cx);
                    }))
            })
            .map(|this| match origin {
                Origin::Onboard(slot) if !self.active && !self.busy => this
                    .cursor_pointer()
                    .on_click(cx.listener(move |page, _, _, cx| {
                        page.activate(slot, cx);
                    })),
                _ if !self.busy => this.cursor_grab(),
                _ => this,
            })
            .children(self.number.map(|number| {
                div()
                    .absolute()
                    .top(px(4.0))
                    .left(px(4.0))
                    .px_1()
                    .rounded(px(3.0))
                    .bg(theme.group_box)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from((u16::from(number) + 1).to_string()))
            }))
            .child(avatar(hsla_of(self.color), self.icon, px(CARD_AVATAR), cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.name.clone()),
                    )
                    .children(shortcut(&self.shortcut, origin, cx)),
            )
            .children(self.default.then(|| default_tag(cx)))
            .child(menu(origin, self.menu, self.busy, cx))
    }
}

impl Render for Moving {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .w(px(PREVIEW_WIDTH))
            .h(px(PREVIEW_HEIGHT))
            .px_4()
            .gap_3()
            .rounded(theme.radius_lg)
            .bg(theme.secondary)
            .border_1()
            .border_color(theme.primary)
            .shadow_lg()
            .opacity(0.92)
            .child(avatar(
                hsla_of(self.color),
                self.icon,
                px(PREVIEW_AVATAR),
                cx,
            ))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.name.clone()),
            )
    }
}

pub(super) fn drop_zone(
    id: &'static str,
    place: Place,
    cx: &Context<ProfilesPage>,
) -> Stateful<Div> {
    let theme = cx.theme();
    let primary = theme.primary;
    let highlight = theme.primary.opacity(0.08);
    h_flex()
        .id(id)
        .w_full()
        .h(px(CARD_HEIGHT))
        .justify_center()
        .rounded(theme.radius_lg)
        .border_2()
        .border_dashed()
        .border_color(theme.border)
        .text_sm()
        .text_color(theme.muted_foreground)
        .child("Drag & drop a profile here")
        .drag_over::<Moving>(move |style, _, _, _| style.border_color(primary).bg(highlight))
        .on_drop(cx.listener(move |page, moving: &Moving, window, cx| {
            page.drop_profile(moving, place, window, cx);
        }))
}

pub(super) fn section(cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .gap_4()
        .p_6()
        .rounded(theme.radius_lg)
        .bg(theme.group_box)
}

pub(super) fn section_header(
    title: &'static str,
    help: &'static str,
    count: Option<(SharedString, SharedString)>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    h_flex()
        .gap_2()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child(title),
        )
        .child(
            div()
                .id(SharedString::from(format!("help-{title}")))
                .text_color(theme.muted_foreground)
                .child(Icon::new(Catalog::CircleQuestionMark).small())
                .tooltip(tip(help)),
        )
        .children(count.map(|(text, tooltip)| {
            div()
                .id(SharedString::from(format!("count-{title}")))
                .px_1p5()
                .h(px(CHIP_HEIGHT))
                .flex()
                .items_center()
                .rounded(px(4.0))
                .bg(theme.secondary)
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.muted_foreground)
                .child(text)
                .tooltip(move |window, cx| {
                    Tooltip::new(tooltip.clone())
                        .max_w(px(TOOLTIP_WIDTH))
                        .build(window, cx)
                })
        }))
}

pub(super) fn grid(cells: Vec<AnyElement>) -> Div {
    let mut rows = v_flex().w_full().gap_3();
    let mut cells = cells.into_iter();
    while let Some(first) = cells.next() {
        let second = cells.next();
        rows = rows.child(
            h_flex()
                .w_full()
                .gap_3()
                .child(div().flex_1().min_w_0().child(first))
                .child(div().flex_1().min_w_0().children(second)),
        );
    }
    rows
}

fn shortcut(chips: &[SharedString], origin: Origin, cx: &App) -> Option<Stateful<Div>> {
    let Origin::Onboard(slot) = origin else {
        return None;
    };
    if chips.is_empty() {
        return None;
    }
    let theme = cx.theme();
    Some(
        h_flex()
            .id(("profile-shortcut", usize::from(slot.get())))
            .flex_none()
            .gap_1()
            .children(chips.iter().map(|chip| {
                div()
                    .px_1p5()
                    .h(px(CHIP_HEIGHT))
                    .flex()
                    .items_center()
                    .rounded(px(4.0))
                    .bg(theme.group_box)
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(chip.clone())
            }))
            .tooltip(tip(
                "Shortcut that switches to this profile. Change it on the Remap page.",
            )),
    )
}

fn default_tag(cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id("default-profile")
        .flex_none()
        .px_1p5()
        .h(px(CHIP_HEIGHT))
        .flex()
        .items_center()
        .rounded(px(4.0))
        .bg(theme.primary)
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(theme.primary_foreground)
        .child("DEFAULT")
        .tooltip(tip(
            "The Mode key returns to this profile, and it cannot be deleted. Any profile moved to the first slot becomes the default.",
        ))
}

fn tip(text: &'static str) -> impl Fn(&mut Window, &mut App) -> AnyView {
    move |window, cx| {
        Tooltip::new(text)
            .max_w(px(TOOLTIP_WIDTH))
            .build(window, cx)
    }
}

fn menu(
    origin: Origin,
    entries: Vec<(Item, super::Command)>,
    busy: bool,
    cx: &Context<ProfilesPage>,
) -> Div {
    let page = cx.entity().downgrade();
    let id: ElementId = match origin {
        Origin::Onboard(slot) => ("onboard-menu", usize::from(slot.get())).into(),
        Origin::Inactive(index) => ("inactive-menu", index).into(),
    };
    let (items, commands): (Vec<Item>, Vec<super::Command>) = entries.into_iter().unzip();
    div()
        .flex_none()
        .w(px(MENU_BUTTON))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            Dropdown::new(id)
                .align_end()
                .menu_width(px(MENU_WIDTH))
                .disabled(busy)
                .items(items)
                .on_select(move |index, window, cx| {
                    if let Some(command) = commands.get(index).copied() {
                        page.update(cx, |page, cx| page.command(origin, command, window, cx))
                            .ok();
                    }
                })
                .trigger(kebab_face),
        )
}

fn kebab_face(open: bool, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let hover = theme.secondary_hover;
    div()
        .size(px(MENU_BUTTON))
        .rounded(theme.radius)
        .flex()
        .items_center()
        .justify_center()
        .when(open, |this| this.bg(hover))
        .hover(move |style| style.bg(hover))
        .child(
            Icon::new(Catalog::EllipsisVertical)
                .small()
                .text_color(theme.muted_foreground),
        )
        .into_any_element()
}
