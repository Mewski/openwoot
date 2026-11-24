use gpui_kit::{
    Context, Div, ParentElement as _, SharedString, Styled as _, Window,
    component::{
        Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
    },
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
    pub(super) fn host_section(&self, cx: &Context<Self>) -> Div {
        if !self
            .capabilities(cx)
            .is_some_and(|capabilities| capabilities.host_signals)
        {
            return div();
        }
        let bound = match &self.host_ids {
            Some(ids) if ids.is_empty() => "none".to_owned(),
            Some(ids) => ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            None => "not read".to_owned(),
        };
        let sent = self
            .host_sent
            .iter()
            .map(|signal| format!("{} = {}", signal.id, signal.value))
            .collect::<Vec<_>>()
            .join(", ");
        panel::section(
            "Host signals",
            Some(
                "Values that programs on the computer send to the light bar by id, for progress \
                 bars and conditions on host signals. Set one by hand here to try an effect; the \
                 keyboard sets every host signal to 0 when it is plugged in again or the \
                 computer restarts."
                    .into(),
            ),
            cx,
        )
        .child(panel::row(
            "Bound host signals",
            Some("The ids the shown effects use, in the order they were bound.".into()),
            h_flex()
                .gap_3()
                .items_center()
                .child(div().text_sm().child(bound))
                .child(
                    Button::new("host-refresh")
                        .label("Refresh")
                        .xsmall()
                        .ghost()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.refresh_host_ids(window, cx);
                        })),
                ),
            cx,
        ))
        .child(panel::row(
            "Set a value",
            None,
            h_flex()
                .gap_2()
                .items_center()
                .child(div().text_sm().child("Id"))
                .child(self.number(Field::HostId))
                .child(div().text_sm().child("Value"))
                .child(self.number(Field::HostValue))
                .child(
                    Button::new("host-send")
                        .label("Send")
                        .small()
                        .primary()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.send_host_value(window, cx);
                        })),
                ),
            cx,
        ))
        .when(!sent.is_empty(), |this| {
            this.child(panel::note(SharedString::from(format!("Sent: {sent}")), cx))
        })
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
