use std::{cell::Cell, rc::Rc};

use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, ElementId, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Pixels, Render,
    RenderOnce, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, WeakEntity,
    Window,
    assets::IconName as Catalog,
    canvas,
    component::{ActiveTheme as _, Icon, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px, transparent_black,
};
use openwoot_protocol::{lighting::Rgb, profile::Slot};

use super::{ProfilesPage, icon::avatar};
use crate::{
    color_picker::hsla_of,
    dropdown::{Dropdown, Item},
    panel::{self, ActionButton, Tip},
    theme,
};

const CARD_HEIGHT: f32 = 72.0;
const CARD_PADDING_X: f32 = 20.0;
const CARD_GAP: f32 = 8.0;
const RADIUS: f32 = 8.0;
const SMALL_RADIUS: f32 = 6.0;
const ACTIVE_RING: f32 = 1.0;
const DROP_RING: f32 = 2.0;
const LIFTED_RING: f32 = 1.0;
const CARD_AVATAR: f32 = 48.0;
const AVATAR_GAP: f32 = 12.0;
const NAME_GAP: f32 = 8.0;
const NAME_TEXT: f32 = 16.0;
const NAME_LINE: f32 = 19.2;
const SLOT_BADGE: f32 = 20.0;
const SLOT_INSET: f32 = 3.0;
const SMALL_TEXT: f32 = 12.0;
const BADGE_LINE: f32 = 18.0;
const MENU_WIDTH: f32 = 224.0;
const CHIP_HEIGHT: f32 = 20.0;
const CHIP_PADDING_X: f32 = 10.0;
const CHIP_GAP: f32 = 4.0;
const CHIP_TEXT: f32 = 12.8;
const COUNT_PADDING: f32 = 4.0;
const COUNT_GAP: f32 = 4.0;
const TAG_PADDING_X: f32 = 4.0;
const SECTION_PADDING: f32 = 16.0;
const SECTION_GAP: f32 = 20.0;
const HEADER_GAP: f32 = 8.0;
const TITLE_TEXT: f32 = 20.0;
const TITLE_LINE: f32 = 24.0;
const GRID_COLUMN_GAP: f32 = 16.0;
const GRID_ROW_GAP: f32 = 12.0;
const APPS_GAP: f32 = 4.0;
const APPS_ICON: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Origin {
    Onboard(Slot),
    Linked(Slot),
    Inactive(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Place {
    Onboard(Slot),
    NewOnboard,
    Linked,
    NewLinked,
    Inactive,
}

#[derive(Debug, Clone)]
pub(super) struct Face {
    pub(super) name: SharedString,
    pub(super) color: Rgb,
    pub(super) icon: u8,
    pub(super) number: Option<u8>,
    pub(super) shortcut: Vec<SharedString>,
    pub(super) apps: Option<SharedString>,
}

#[derive(Debug, Clone)]
pub(super) struct Moving {
    pub(super) origin: Origin,
    face: Face,
    width: Rc<Cell<Pixels>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Status {
    Ready,
    Lifted,
    Busy,
}

pub(super) struct Card {
    pub(super) origin: Origin,
    pub(super) face: Face,
    pub(super) active: bool,
    pub(super) default: bool,
    pub(super) menu: Vec<(Item, super::Command)>,
    pub(super) status: Status,
}

#[derive(IntoElement)]
pub(super) struct Tipped {
    element: Stateful<Div>,
    key: ElementId,
    tip: Tip,
    beside: bool,
}

impl Origin {
    const fn place(self) -> Place {
        match self {
            Self::Onboard(slot) => Place::Onboard(slot),
            Self::Linked(_) => Place::Linked,
            Self::Inactive(_) => Place::Inactive,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Onboard(slot) | Self::Linked(slot) => usize::from(slot.index()),
            Self::Inactive(index) => index,
        }
    }
}

impl Place {
    pub(super) fn accepts(self, origin: Origin) -> bool {
        match (origin, self) {
            (Origin::Onboard(from), Self::Onboard(to)) => from != to,
            (Origin::Onboard(_), Self::Inactive | Self::NewLinked)
            | (Origin::Inactive(_), Self::Onboard(_) | Self::NewOnboard | Self::NewLinked)
            | (Origin::Linked(_), Self::Inactive | Self::NewOnboard) => true,
            _ => false,
        }
    }
}

impl Card {
    pub(super) fn render(self, cx: &Context<ProfilesPage>) -> Stateful<Div> {
        let theme = cx.theme();
        let origin = self.origin;
        let place = origin.place();
        let width = Rc::new(Cell::new(px(0.0)));
        let busy = self.status == Status::Busy;
        let moving = Moving {
            origin,
            face: self.face.clone(),
            width: Rc::clone(&width),
        };
        let page = cx.entity().downgrade();
        let hover = theme.secondary;
        let primary = theme.primary;
        h_flex()
            .id(("profile-card", origin.index()))
            .relative()
            .w_full()
            .h(px(CARD_HEIGHT))
            .px(px(CARD_PADDING_X))
            .gap(px(CARD_GAP))
            .rounded(px(RADIUS))
            .bg(if self.active {
                theme.secondary
            } else {
                theme.accent
            })
            .when(self.status == Status::Lifted, gpui_kit::Styled::invisible)
            .when(!self.active, |this| {
                this.hover(move |style| style.bg(hover))
            })
            .child(
                canvas(
                    move |bounds, _, _| width.set(bounds.size.width),
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .when(self.active, |this| this.child(ring(ACTIVE_RING, primary)))
            .when(!busy, |this| {
                this.on_drag(moving, move |moving, _, _, cx| {
                    page.update(cx, |page, cx| page.lift(moving.origin, cx))
                        .ok();
                    cx.new(|_| moving.clone())
                })
                .drag_over::<Moving>(move |style, moving, _, _| {
                    if place.accepts(moving.origin) {
                        style.bg(hover)
                    } else {
                        style
                    }
                })
                .on_drop(cx.listener(move |page, moving: &Moving, window, cx| {
                    page.drop_profile(moving, place, window, cx);
                }))
                .child(
                    ring(DROP_RING, transparent_black())
                        .border_dashed()
                        .drag_over::<Moving>(move |style, moving, _, _| {
                            if place.accepts(moving.origin) {
                                style.border_color(primary)
                            } else {
                                style
                            }
                        }),
                )
            })
            .map(|this| match origin {
                Origin::Onboard(slot) | Origin::Linked(slot) if !self.active && !busy => this
                    .cursor_pointer()
                    .on_click(cx.listener(move |page, _, _, cx| {
                        page.activate(slot, cx);
                    })),
                _ => this,
            })
            .children(self.face.number.map(|number| slot_badge(number, cx)))
            .child(identity(&self.face, true, cx))
            .child(
                h_flex()
                    .flex_none()
                    .gap(px(CARD_GAP))
                    .children(self.default.then(|| default_tag(cx)))
                    .child(menu(origin, self.menu, busy, cx)),
            )
    }
}

impl Render for Moving {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .relative()
            .w(self.width.get())
            .h(px(CARD_HEIGHT))
            .px(px(CARD_PADDING_X))
            .gap(px(CARD_GAP))
            .rounded(px(RADIUS))
            .bg(theme::palette().tooltip)
            .child(ring(LIFTED_RING, theme.secondary_active))
            .children(self.face.number.map(|number| slot_badge(number, cx)))
            .child(identity(&self.face, false, cx))
    }
}

pub(super) fn drop_zone(
    id: &'static str,
    place: Place,
    cx: &Context<ProfilesPage>,
) -> Stateful<Div> {
    let theme = cx.theme();
    let primary = theme.primary;
    h_flex()
        .id(id)
        .w_full()
        .h(px(CARD_HEIGHT))
        .px(px(CARD_PADDING_X))
        .justify_center()
        .rounded(px(RADIUS))
        .border_2()
        .border_dashed()
        .border_color(theme.border)
        .text_size(px(NAME_TEXT))
        .line_height(px(NAME_LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.muted_foreground)
        .child(if place == Place::NewLinked {
            "Drag & drop a profile here to link it"
        } else {
            "Drag & drop a profile here"
        })
        .drag_over::<Moving>(move |style, moving, _, _| {
            if place.accepts(moving.origin) {
                style.border_color(primary).text_color(primary)
            } else {
                style
            }
        })
        .on_drop(cx.listener(move |page, moving: &Moving, window, cx| {
            page.drop_profile(moving, place, window, cx);
        }))
}

pub(super) fn section(cx: &App) -> Div {
    v_flex()
        .w_full()
        .gap(px(SECTION_GAP))
        .p(px(SECTION_PADDING))
        .rounded(px(RADIUS))
        .bg(cx.theme().sidebar)
}

pub(super) fn section_header(
    title: &'static str,
    help: &'static str,
    count: Option<(usize, usize, SharedString)>,
    cx: &App,
) -> Div {
    let owner = SharedString::from(title);
    h_flex()
        .gap(px(HEADER_GAP))
        .child(
            div()
                .text_size(px(TITLE_TEXT))
                .line_height(px(TITLE_LINE))
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().group_box_foreground)
                .child(title),
        )
        .child(panel::help(&owner, SharedString::from(help), cx))
        .children(count.map(|(used, capacity, tooltip)| {
            tipped(
                count_chip(used, capacity, cx).id("profile-count"),
                "profile-count-tip",
                Tip::Text(tooltip),
            )
        }))
}

pub(in crate::page) fn count_chip(used: usize, capacity: usize, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .flex_none()
        .h(px(CHIP_HEIGHT))
        .p(px(COUNT_PADDING))
        .gap(px(COUNT_GAP))
        .rounded(px(SMALL_RADIUS))
        .bg(theme.secondary)
        .text_size(px(SMALL_TEXT))
        .line_height(px(SMALL_TEXT))
        .text_color(theme.sidebar_foreground)
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.group_box_foreground)
                .child(SharedString::from(used.to_string())),
        )
        .child("/")
        .child(SharedString::from(capacity.to_string()))
}

pub(super) fn grid(
    id: &'static str,
    cells: Vec<AnyElement>,
    mut trailing: Option<AnyElement>,
) -> Stateful<Div> {
    let mut rows = v_flex().id(id).w_full().gap(px(GRID_ROW_GAP));
    let mut cells = cells.into_iter();
    while let Some(first) = cells.next() {
        let second = cells.next().or_else(|| trailing.take());
        rows = rows.child(
            h_flex()
                .w_full()
                .gap(px(GRID_COLUMN_GAP))
                .child(div().flex_1().min_w_0().child(first))
                .child(div().flex_1().min_w_0().children(second)),
        );
    }
    rows.children(trailing)
}

fn ring(width: f32, color: Hsla) -> Div {
    div()
        .absolute()
        .top(px(-width))
        .left(px(-width))
        .right(px(-width))
        .bottom(px(-width))
        .rounded(px(RADIUS + width))
        .border(px(width))
        .border_color(color)
}

fn slot_badge(number: u8, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .absolute()
        .top(px(SLOT_INSET))
        .left(px(SLOT_INSET))
        .size(px(SLOT_BADGE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(RADIUS))
        .bg(theme.secondary)
        .text_size(px(SMALL_TEXT))
        .line_height(px(BADGE_LINE))
        .font_weight(FontWeight::BOLD)
        .text_color(theme.group_box_foreground)
        .child(SharedString::from((u16::from(number) + 1).to_string()))
}

fn identity(face: &Face, interactive: bool, cx: &App) -> Div {
    let theme = cx.theme();
    let name = h_flex()
        .w_full()
        .min_w_0()
        .gap(px(NAME_GAP))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(NAME_TEXT))
                .line_height(px(NAME_LINE))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.foreground)
                .child(face.name.clone()),
        )
        .children(shortcut(face, interactive, cx));
    h_flex()
        .flex_1()
        .min_w_0()
        .gap(px(AVATAR_GAP))
        .child(avatar(hsla_of(face.color), face.icon, px(CARD_AVATAR), cx))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(name)
                .children(face.apps.clone().map(|apps| {
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .gap(px(APPS_GAP))
                        .text_color(theme.muted_foreground)
                        .child(Icon::new(Catalog::Link).flex_none().size(px(APPS_ICON)))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(NAME_TEXT))
                                .line_height(px(NAME_LINE))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(apps),
                        )
                })),
        )
}

fn shortcut(face: &Face, interactive: bool, cx: &App) -> Option<AnyElement> {
    if face.shortcut.is_empty() {
        return None;
    }
    let theme = cx.theme();
    let chips = h_flex()
        .flex_none()
        .gap(px(CHIP_GAP))
        .children(face.shortcut.iter().map(|chip| {
            div()
                .flex_none()
                .px(px(CHIP_PADDING_X))
                .h(px(CHIP_HEIGHT))
                .flex()
                .items_center()
                .rounded(px(RADIUS))
                .bg(theme.secondary)
                .font_family(theme.mono_font_family.clone())
                .text_size(px(CHIP_TEXT))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.foreground)
                .whitespace_nowrap()
                .child(chip.clone())
        }));
    Some(if interactive {
        tipped(
            chips.id("profile-shortcut"),
            "profile-shortcut-tip",
            Tip::Help {
                title: "Onboard Profile Shortcuts".into(),
                text: "Use this shortcut to quickly swap to this profile.".into(),
            },
        )
        .into_any_element()
    } else {
        chips.into_any_element()
    })
}

fn default_tag(cx: &App) -> Tipped {
    let theme = cx.theme();
    tipped(
        div()
            .id("default-profile")
            .flex_none()
            .px(px(TAG_PADDING_X))
            .h(px(CHIP_HEIGHT))
            .flex()
            .items_center()
            .rounded(px(SMALL_RADIUS))
            .bg(theme.primary)
            .text_size(px(SMALL_TEXT))
            .font_weight(FontWeight::BOLD)
            .text_color(theme.primary_foreground)
            .child("DEFAULT"),
        "default-profile-tip",
        Tip::Text("The Mode key returns to this profile".into()),
    )
}

pub(super) fn tipped(element: Stateful<Div>, key: impl Into<ElementId>, tip: Tip) -> Tipped {
    Tipped {
        element,
        key: key.into(),
        tip,
        beside: false,
    }
}

pub(super) fn tipped_beside(element: Stateful<Div>, key: impl Into<ElementId>, tip: Tip) -> Tipped {
    Tipped {
        beside: true,
        ..tipped(element, key, tip)
    }
}

fn menu(
    origin: Origin,
    entries: Vec<(Item, super::Command)>,
    busy: bool,
    cx: &Context<ProfilesPage>,
) -> Div {
    div()
        .flex_none()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(options(origin, entries, busy, cx.entity().downgrade()))
}

pub(super) fn options(
    origin: Origin,
    entries: Vec<(Item, super::Command)>,
    busy: bool,
    page: WeakEntity<ProfilesPage>,
) -> Dropdown {
    let (items, commands): (Vec<Item>, Vec<super::Command>) = entries.into_iter().unzip();
    Dropdown::new(("profile-menu", origin.index()))
        .menu_width(px(MENU_WIDTH))
        .disabled(busy)
        .items(items)
        .on_select(move |index, window, cx| {
            if let Some(command) = commands.get(index).copied() {
                page.update(cx, |page, cx| page.command(origin, command, window, cx))
                    .ok();
            }
        })
        .trigger(kebab_face)
}

fn kebab_face(open: bool, _: &App) -> AnyElement {
    ActionButton::icon_only("profile-options", Icon::new(Catalog::EllipsisVertical))
        .plain()
        .tooltip("Profile Options")
        .selected(open)
        .into_any_element()
}

impl RenderOnce for Tipped {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.beside {
            panel::with_tip_right(self.element, self.key, self.tip, window, cx)
        } else {
            panel::with_tip(self.element, self.key, self.tip, window, cx)
        }
    }
}
