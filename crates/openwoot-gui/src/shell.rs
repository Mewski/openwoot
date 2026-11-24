use std::collections::HashMap;

use gpui_kit::{
    AnyView, Context, Div, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariants},
        h_flex,
        notification::Notification,
        sidebar::{
            Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem,
        },
        spinner::Spinner,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px, rgb,
};
use openwoot_protocol::profile::Slot;

use crate::{
    page::{Group, Page},
    store::{Link, Store, StoreEvent},
};

const SIDEBAR_WIDTH: f32 = 232.0;

pub(crate) struct Shell {
    store: Entity<Store>,
    page: Page,
    pages: HashMap<Page, AnyView>,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscription = cx.subscribe_in(&store, window, |this, _, event, window, cx| {
            match event {
                StoreEvent::Failed(message) => {
                    window.push_notification(Notification::error(message.clone()), cx);
                }
                StoreEvent::Disconnected => {
                    window.push_notification(
                        Notification::warning("The keyboard was disconnected"),
                        cx,
                    );
                }
                StoreEvent::Connected => {
                    if !this.page.is_available(this.store.read(cx).keyboard()) {
                        this.page = Page::Performance;
                    }
                }
                StoreEvent::ProfileChanged | StoreEvent::Reloaded => {}
            }
            cx.notify();
        });
        let observation = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            store,
            page: Page::Performance,
            pages: HashMap::new(),
            _subscriptions: vec![subscription, observation],
        }
    }

    fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    fn view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        let store = self.store.clone();
        self.pages
            .entry(self.page)
            .or_insert_with(|| self.page.open(&store, window, cx))
            .clone()
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);
        let available: Vec<Page> = Page::ALL
            .into_iter()
            .filter(|page| page.is_available(store.keyboard()))
            .collect();
        let status = status_line(
            store.link(),
            store.keyboard().map(|keyboard| {
                SharedString::from(format!(
                    "{} · firmware {}",
                    keyboard.name, keyboard.version.firmware
                ))
            }),
        );
        let current = self.page;
        let menu = |group: Group, cx: &mut Context<Self>| {
            SidebarMenu::new().children(
                available
                    .iter()
                    .copied()
                    .filter(|page| page.group() == group)
                    .map(|page| {
                        SidebarMenuItem::new(page.title())
                            .icon(page.icon())
                            .active(current == page)
                            .on_click(cx.listener(move |this, _, _, cx| this.show(page, cx)))
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let keyboard_menu = menu(Group::Keyboard, cx);
        let device_menu = menu(Group::Device, cx);
        Sidebar::new("navigation")
            .w(px(SIDEBAR_WIDTH))
            .header(
                SidebarHeader::new().child(
                    h_flex()
                        .gap_2()
                        .child(Icon::new(Catalog::Keyboard))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("openwoot")),
                ),
            )
            .child(SidebarGroup::new("Keyboard").child(keyboard_menu))
            .child(SidebarGroup::new("Device").child(device_menu))
            .footer(SidebarFooter::new().child(status))
    }

    fn top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let store = self.store.read(cx);
        let Some(keyboard) = store.keyboard() else {
            return h_flex()
                .h(px(52.0))
                .px_4()
                .border_b_1()
                .border_color(theme.border);
        };
        let active = keyboard.active;
        let unsaved = keyboard.unsaved;
        let profiles: Vec<(Slot, SharedString, gpui_kit::Hsla)> = keyboard
            .profiles
            .iter()
            .enumerate()
            .filter_map(|(index, info)| {
                let slot = Slot::new(u8::try_from(index).ok()?);
                let color = rgb(u32::from_be_bytes([
                    0,
                    info.color.red,
                    info.color.green,
                    info.color.blue,
                ]));
                Some((slot, info.name.as_str().to_owned().into(), color.into()))
            })
            .collect();
        h_flex()
            .h(px(52.0))
            .px_4()
            .gap_2()
            .justify_between()
            .border_b_1()
            .border_color(theme.border)
            .child(
                h_flex().gap_1().children(profiles.into_iter().map(|(slot, name, color)| {
                    Button::new(SharedString::from(format!("profile-{}", slot.get())))
                        .small()
                        .when(slot == active, ButtonVariants::primary)
                        .when(slot != active, ButtonVariants::ghost)
                        .child(
                            h_flex()
                                .gap_1p5()
                                .child(div().size(px(9.0)).rounded_full().bg(color))
                                .child(name),
                        )
                        .tooltip(if slot == active {
                            "Active profile"
                        } else {
                            "Activate this profile; unsaved changes of the active profile are saved first"
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if slot != active {
                                this.store.update(cx, |store, cx| store.activate(slot, cx)).detach();
                            }
                        }))
                })),
            )
            .child(
                h_flex()
                    .gap_2()
                    .when(unsaved, |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(theme.warning)
                                .child("Unsaved changes"),
                        )
                    })
                    .child(
                        Button::new("revert")
                            .small()
                            .ghost()
                            .label("Discard")
                            .disabled(!unsaved)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.store.update(cx, |store, cx| store.revert(cx)).detach();
                            })),
                    )
                    .child(
                        Button::new("save")
                            .small()
                            .primary()
                            .icon(IconName::Check)
                            .label("Save to keyboard")
                            .disabled(!unsaved)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let saved = this.store.update(cx, |store, cx| store.save(cx));
                                cx.spawn_in(window, async move |_, cx| {
                                    if saved.await.is_ok() {
                                        cx.update(|window, cx| {
                                            window.push_notification(Notification::success("Saved to the keyboard"), cx);
                                        })
                                        .ok();
                                    }
                                })
                                .detach();
                            })),
                    ),
            )
    }

    fn waiting(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (title, detail): (SharedString, SharedString) = match self.store.read(cx).link() {
            Link::Searching => (
                "Connect an openwoot keyboard".into(),
                "Plug in a keyboard running openwoot. Keyboards with the stock firmware need openwoot flashed first (see Firmware).".into(),
            ),
            Link::Loading => ("Reading the keyboard".into(), "One moment…".into()),
            Link::Connected => ("Connected".into(), SharedString::default()),
            Link::Unreachable(reason) => ("Cannot use the keyboard".into(), reason.clone()),
        };
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .child(Spinner::new().large())
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .max_w(px(520.0))
                    .text_center()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(detail),
            )
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.store.read(cx).keyboard().is_some();
        let content = if connected || !self.page.needs_keyboard() {
            div()
                .id("page")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(self.view(window, cx))
                .into_any_element()
        } else {
            self.waiting(cx).into_any_element()
        };
        h_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.sidebar(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.top_bar(cx))
                    .child(content),
            )
    }
}

fn status_line(link: &Link, connected: Option<SharedString>) -> Div {
    let (icon, text): (IconName, SharedString) = match (link, connected) {
        (_, Some(description)) => (IconName::CircleCheck, description),
        (Link::Loading, None) => (IconName::Loader, "Reading the keyboard".into()),
        (Link::Unreachable(_), None) => (IconName::TriangleAlert, "Keyboard unreachable".into()),
        (Link::Searching | Link::Connected, None) => {
            (IconName::Search, "Looking for a keyboard".into())
        }
    };
    h_flex()
        .gap_2()
        .text_xs()
        .child(Icon::new(icon).small())
        .child(div().flex_1().min_w_0().child(text))
}
