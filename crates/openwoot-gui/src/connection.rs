use std::rc::Rc;

use gpui_kit::{
    App, Context, Entity, FontWeight, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window,
    component::{ActiveTheme as _, Sizable as _, h_flex, spinner::Spinner, v_flex},
    div,
    prelude::FluentBuilder as _,
};
use openwoot_protocol::device::Layout;

use crate::{
    panel::ActionButton,
    store::{Keyboard, Link, Store},
};

type Navigate = Rc<dyn Fn(&mut Window, &mut App)>;

const NOT_FOUND_STEPS: [&str; 4] = [
    "Connect the keyboard with a data cable.",
    "On stock firmware or in update mode, install openwoot first.",
    "Close Wootility.",
    "On Linux, install the udev rules (70-openwoot.rules).",
];

pub(crate) struct ConnectionPanel {
    store: Entity<Store>,
    open_firmware: Navigate,
    _observation: Subscription,
}

impl ConnectionPanel {
    pub(crate) fn new(
        store: Entity<Store>,
        open_firmware: impl Fn(&mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        let observation = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            store,
            open_firmware: Rc::new(open_firmware),
            _observation: observation,
        }
    }

    fn connected(keyboard: &Keyboard, cx: &App) -> gpui_kit::Div {
        let capabilities = &keyboard.capabilities;
        let layout = keyboard
            .layout
            .map_or("not reported", |layout| match layout {
                Layout::Ansi => "ANSI",
                Layout::Iso => "ISO",
                Layout::Jis => "JIS",
            });
        let rows: [(&str, SharedString); 5] = [
            ("Keyboard", keyboard.name.clone()),
            (
                "Firmware",
                format!("openwoot {}", keyboard.version.firmware).into(),
            ),
            (
                "Protocol",
                format!("version {}", keyboard.version.protocol).into(),
            ),
            ("Layout", layout.into()),
            (
                "Profiles",
                format!("{} of {}", keyboard.profiles.len(), capabilities.profiles).into(),
            ),
        ];
        v_flex()
            .gap_2()
            .children(rows.into_iter().map(|(label, value)| {
                h_flex()
                    .justify_between()
                    .gap_4()
                    .text_sm()
                    .child(div().text_color(cx.theme().muted_foreground).child(label))
                    .child(value)
            }))
    }

    fn absent(&self, link: &Link, searching: bool, cx: &mut Context<Self>) -> gpui_kit::Div {
        let theme = cx.theme();
        let (title, reason): (&str, Option<SharedString>) = match link {
            Link::Unreachable(reason) => ("Keyboard unavailable", Some(reason.clone())),
            Link::Loading => ("Connecting", None),
            Link::Searching | Link::Connected => ("No keyboard found", None),
        };
        let open_firmware = self.open_firmware.clone();
        v_flex()
            .gap_4()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
            .when_some(reason, |this, reason| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(reason),
                )
            })
            .child(
                v_flex()
                    .gap_1p5()
                    .children(
                        NOT_FOUND_STEPS
                            .into_iter()
                            .enumerate()
                            .map(|(index, step)| {
                                h_flex()
                                    .items_start()
                                    .gap_2()
                                    .text_sm()
                                    .child(
                                        div()
                                            .text_color(theme.muted_foreground)
                                            .child(format!("{}.", index + 1)),
                                    )
                                    .child(div().flex_1().child(step))
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .when(searching, |this| this.child(Spinner::new().small()))
                    .child(
                        ActionButton::new("open-firmware", "Install firmware")
                            .on_click(move |_, window, cx| open_firmware(window, cx)),
                    )
                    .child(
                        ActionButton::new(
                            "search-now",
                            if searching {
                                "Searching"
                            } else {
                                "Search again"
                            },
                        )
                        .primary()
                        .disabled(searching)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.store.update(cx, Store::search);
                        })),
                    ),
            )
    }
}

impl Render for ConnectionPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);
        if let Some(keyboard) = store.keyboard() {
            return Self::connected(keyboard, cx);
        }
        let link = store.link().clone();
        let searching = store.is_searching();
        self.absent(&link, searching, cx)
    }
}
