use gpui_kit::{
    Context, Div, ParentElement as _, SharedString, Styled as _,
    component::{
        ActiveTheme as _, Disableable as _, h_flex, slider::Slider, switch::Switch, v_flex,
    },
    div, px,
};
use openwoot_protocol::{
    lightbar::{self, Entries, Entry},
    lighting::Rgb,
    profile::Slot,
};

use super::{
    Chosen, LightbarPage, Owner,
    preview::{self, Profile},
};
use crate::{
    page::lighting::{hsla_of, level_of, percent_of},
    panel,
};

const LED_WIDTH: f32 = 30.0;
const LED_HEIGHT: f32 = 14.0;
const SLIDER_WIDTH: f32 = 280.0;

impl LightbarPage {
    pub(super) fn bar_section(&self, cx: &Context<Self>) -> Div {
        let settings = self.settings.as_ref();
        panel::section(
            "Light bar",
            Some(
                "The keyboard's Light Indicator. Profile effects draw over the global effects, \
                 and every effect over the ones before it."
                    .into(),
            ),
            cx,
        )
        .child(self.toggle_row(
            "lightbar-enabled",
            "Light bar on",
            "Turn the light bar off for this profile.",
            settings.is_some_and(|settings| settings.enabled),
            |settings, enabled| settings.enabled = enabled,
            cx,
        ))
        .child(self.toggle_row(
            "lightbar-use-global",
            "Inherit global effects",
            "Show the global effects in this profile, below its own effects.",
            settings.is_some_and(|settings| settings.use_global),
            |settings, enabled| settings.use_global = enabled,
            cx,
        ))
        .child(self.brightness_rows(cx))
        .child(self.preview(cx))
    }

    fn toggle_row(
        &self,
        id: &'static str,
        label: &'static str,
        description: &'static str,
        checked: bool,
        change: fn(&mut lightbar::Settings, bool),
        cx: &Context<Self>,
    ) -> Div {
        panel::row(
            label,
            Some(description.into()),
            Switch::new(id)
                .checked(checked)
                .disabled(self.settings.is_none())
                .on_change(cx.listener(move |this, enabled: &bool, window, cx| {
                    let enabled = *enabled;
                    this.edit_settings(|settings| change(settings, enabled), window, cx);
                })),
            cx,
        )
    }

    fn brightness_rows(&self, cx: &Context<Self>) -> Div {
        let brightness = self.settings.as_ref().map(|settings| settings.brightness);
        let following = brightness.is_some_and(|level| level.is_none());
        let label = match brightness {
            Some(Some(level)) => format!("Brightness {} %", percent_of(level)),
            _ => "Brightness".to_owned(),
        };
        v_flex()
            .child(panel::row(
                "Follow key brightness",
                Some("Light the bar as bright as the keys.".into()),
                Switch::new("lightbar-follow-keys")
                    .checked(following)
                    .disabled(brightness.is_none())
                    .on_change(cx.listener(|this, following: &bool, window, cx| {
                        let level = if *following {
                            None
                        } else {
                            Some(level_of(this.brightness.read(cx).value().end()))
                        };
                        this.edit_settings(|settings| settings.brightness = level, window, cx);
                    })),
                cx,
            ))
            .child(panel::row(
                SharedString::from(label),
                None,
                div().w(px(SLIDER_WIDTH)).child(
                    Slider::new(&self.brightness).disabled(brightness.is_none() || following),
                ),
                cx,
            ))
    }

    fn preview(&self, cx: &Context<Self>) -> Div {
        let Some(settings) = &self.settings else {
            return div();
        };
        let leds = self.leds(cx);
        let profile = self.preview_profile(cx);
        let mut drawn = Vec::new();
        if settings.use_global
            && let Some(global) = &self.global
        {
            drawn.extend(shown(global, Owner::Global, self.chosen));
        }
        drawn.extend(shown(&settings.entries, Owner::Profile, self.chosen));
        let colors = if settings.enabled {
            preview::paint(drawn, leds, profile)
        } else {
            vec![Rgb::default(); usize::from(leds)]
        };
        let theme = cx.theme();
        v_flex()
            .gap_2()
            .pt_2()
            .child(h_flex().gap_1().children(colors.into_iter().map(|color| {
                div()
                    .w(px(LED_WIDTH))
                    .h(px(LED_HEIGHT))
                    .rounded(px(3.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(hsla_of(color))
            })))
            .child(panel::note(
                "Preview of the static colours: effects with a condition show only while \
                 selected, progress bars half full, pulses in their second colour and synced \
                 keys in grey; the brightness is left out.",
                cx,
            ))
    }

    fn preview_profile(&self, cx: &Context<Self>) -> Profile {
        let keyboard = self.store.read(cx).keyboard();
        let slot = self.profile.map_or(0, Slot::get);
        let color = keyboard
            .and_then(|keyboard| keyboard.profiles.get(usize::from(slot)))
            .map_or_else(Rgb::default, |info| info.color);
        let count = keyboard.map_or(1, |keyboard| {
            u8::try_from(keyboard.profiles.len()).unwrap_or(u8::MAX)
        });
        Profile { slot, count, color }
    }
}

fn shown(entries: &Entries, owner: Owner, chosen: Option<Chosen>) -> impl Iterator<Item = &Entry> {
    entries
        .iter()
        .enumerate()
        .filter(move |(index, entry)| {
            entry.condition.is_none()
                || chosen
                    == Some(Chosen {
                        owner,
                        index: *index,
                    })
        })
        .map(|(_, entry)| entry)
}
