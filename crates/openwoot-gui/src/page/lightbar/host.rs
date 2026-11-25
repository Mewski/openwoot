use gpui_kit::{
    Context, Div, ParentElement as _, SharedString, Styled as _, Window,
    component::{Icon, IconName, h_flex},
    div,
    prelude::FluentBuilder as _,
};
use openwoot_protocol::{
    endpoints::{GetHostSignals, SetHostSignals},
    lightbar::{HostSignal, HostSignals},
};

use super::{
    LightbarPage,
    number::{Field, Number},
};
use crate::{panel, store::Scope};

impl LightbarPage {
    pub(super) fn host_card(&self, cx: &Context<Self>) -> Option<Div> {
        let supported = self
            .capabilities(cx)
            .is_some_and(|capabilities| capabilities.host_signals);
        if !supported || self.settings.is_none() {
            return None;
        }
        let bound = match &self.host_ids {
            Some(ids) if !ids.is_empty() => ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            Some(_) | None => "None".to_owned(),
        };
        let sent = self
            .host_sent
            .iter()
            .map(|signal| format!("{} = {}", signal.id, signal.value))
            .collect::<Vec<_>>()
            .join(", ");
        Some(
            panel::section("Host Signals", Some("Values sent by programs".into()), cx)
                .child(panel::toggle(
                    "Used by effects",
                    None,
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().text_sm().child(bound))
                        .child(
                            panel::ActionButton::icon_only(
                                "host-refresh",
                                Icon::new(IconName::RefreshCw),
                            )
                            .tooltip("Refresh")
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.refresh_host_ids(window, cx);
                                },
                            )),
                        ),
                    cx,
                ))
                .child(panel::field(
                    "Send a value",
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(self.number(Field::HostId, cx))
                        .child(self.number(Field::HostValue, cx))
                        .child(panel::ActionButton::new("host-send", "Send").on_click(
                            cx.listener(|this, _, window, cx| {
                                this.send_host_value(window, cx);
                            }),
                        )),
                    cx,
                ))
                .when(!sent.is_empty(), |this| {
                    this.child(panel::note(SharedString::from(format!("Sent {sent}")), cx))
                }),
        )
    }

    pub(super) fn refresh_host_ids(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let generation = self.generation;
        let ids = self
            .store
            .update(cx, |store, cx| store.fetch::<GetHostSignals, _>((), cx));
        cx.spawn_in(window, async move |this, cx| {
            let ids = ids.await.ok();
            this.update(cx, |page, cx| {
                if page.generation == generation {
                    page.host_ids = ids;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn send_host_value(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let read = |field: Field, page: &Self, cx: &Context<Self>| {
            let text = page
                .field(field)
                .map_or_else(SharedString::default, |state| state.read(cx).value());
            field.parse(&text)
        };
        let (id, value) = match (
            read(Field::HostId, self, cx),
            read(Field::HostValue, self, cx),
        ) {
            (Ok(Number::Id(id)), Ok(Number::Value(value))) => (id, value),
            (Err(mistake), _) | (_, Err(mistake)) => {
                self.problem = Some(mistake.into());
                cx.notify();
                return;
            }
            _ => return,
        };
        let signal = HostSignal { id, value };
        let mut signals = HostSignals::new();
        signals.push(signal).ok();
        let sent = self.store.update(cx, |store, cx| {
            store.apply::<SetHostSignals>(signals, Scope::Runtime, cx)
        });
        self.problem = None;
        cx.spawn_in(window, async move |this, cx| {
            if sent.await.is_ok() {
                this.update(cx, |page, cx| {
                    page.host_sent.retain(|kept| kept.id != signal.id);
                    page.host_sent.push(signal);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        cx.notify();
    }
}
