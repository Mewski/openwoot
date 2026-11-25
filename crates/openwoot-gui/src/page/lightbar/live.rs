use std::time::Duration;

use gpui_kit::Context;
use openwoot_protocol::{
    configuration::Rejection, endpoints::GetLightbarColors, lightbar, lighting::Rgb,
};

use super::LightbarPage;
use crate::device::Failure;

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const VISIBLE_WITHIN: Duration = Duration::from_millis(500);
const KEEP_ALIVE: Duration = Duration::from_millis(200);

impl LightbarPage {
    pub(super) fn watch_bar(&mut self, cx: &mut Context<Self>) {
        if self.watch.is_some() {
            return;
        }
        let device = self.store.read(cx).device();
        self.watch = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let Ok(visible) =
                    this.update(cx, |page, _| page.rendered_at.elapsed() < VISIBLE_WITHIN)
                else {
                    return;
                };
                if !visible {
                    continue;
                }
                let colors = match device
                    .configure::<GetLightbarColors, lightbar::Colors>(())
                    .await
                {
                    Ok(colors) => Some(colors),
                    Err(Failure::Rejected(Rejection::Unsupported)) => return,
                    Err(_) => None,
                };
                if this
                    .update(cx, |page, cx| page.take_live(colors, cx))
                    .is_err()
                {
                    return;
                }
            }
        }));
    }

    pub(super) fn stop_watching(&mut self) {
        self.watch = None;
        self.live = None;
    }

    pub(super) fn live_colors(&self, leds: u8) -> Option<Vec<Rgb>> {
        let live = self.live.as_ref()?;
        Some(
            (0..usize::from(leds))
                .map(|led| live.get(led).copied().unwrap_or_default())
                .collect(),
        )
    }

    fn take_live(&mut self, colors: Option<lightbar::Colors>, cx: &mut Context<Self>) {
        if self.live != colors {
            self.live = colors;
            cx.notify();
        } else if self.rendered_at.elapsed() > KEEP_ALIVE {
            cx.notify();
        }
    }
}
