use gpui_kit::{
    Context, Div, ParentElement as _, SharedString, Styled as _, Window,
    component::{h_flex, switch::Switch},
    div,
    prelude::FluentBuilder as _,
    px,
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
    page::lighting::{hsla_of, level_of, wide_slider},
    panel, theme,
};

const LED_WIDTH: f32 = 72.0;
const LED_HEIGHT: f32 = 12.0;
const LED_GAP: f32 = 8.0;
const CASE_RADIUS: f32 = 16.0;
const CASE_LIP: f32 = 10.0;
const CASE_INSET: f32 = 40.0;

impl LightbarPage {
    pub(super) fn bar_card(&self, cx: &Context<Self>) -> Option<Div> {
        let settings = self.settings.as_ref()?;
        let following = settings.brightness.is_none();
        let brightness = self.brightness.read(cx).value().end();
        Some(
            panel::section("Light bar", None, cx)
                .child(Self::toggle_row(
                    "lightbar-enabled",
                    "Enabled",
                    None,
                    settings.enabled,
                    |settings, enabled| settings.enabled = enabled,
                    cx,
                ))
                .child(Self::toggle_row(
                    "lightbar-use-global",
                    "Global effects",
                    Some("Show the global effects below this profile's own."),
                    settings.use_global,
                    |settings, enabled| settings.use_global = enabled,
                    cx,
                ))
                .child(panel::toggle(
                    "Follow keys",
                    Some("Use the key brightness.".into()),
                    Switch::new("lightbar-follow-keys")
                        .checked(following)
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
                .when(!following, |this| {
                    this.child(panel::field(
                        "Brightness",
                        wide_slider(&self.brightness, format!("{brightness:.0} %"), cx),
                        cx,
                    ))
                }),
        )
    }

    fn toggle_row(
        id: &'static str,
        label: &'static str,
        help: Option<&'static str>,
        checked: bool,
        change: fn(&mut lightbar::Settings, bool),
        cx: &Context<Self>,
    ) -> Div {
        panel::toggle(
            label,
            help.map(SharedString::from),
            Switch::new(id).checked(checked).on_change(cx.listener(
                move |this, enabled: &bool, window, cx| {
                    let enabled = *enabled;
                    this.edit_settings(|settings| change(settings, enabled), window, cx);
                },
            )),
            cx,
        )
    }

    pub(super) fn preview(&self, window: &Window, cx: &Context<Self>) -> Option<Div> {
        let settings = self.settings.as_ref()?;
        let leds = self.leds(cx);
        let count = f32::from(leds.max(1));
        let room = f32::from(panel::content_width(window)) - CASE_INSET - LED_GAP * (count - 1.0);
        let led_width = (room / count).min(LED_WIDTH);
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
        let caps = theme::keycaps();
        let leds = h_flex()
            .gap(px(LED_GAP))
            .children(colors.into_iter().map(|color| {
                div()
                    .w(px(led_width))
                    .h(px(LED_HEIGHT))
                    .rounded(px(3.0))
                    .bg(if color == Rgb::default() {
                        caps.top
                    } else {
                        hsla_of(color)
                    })
            }));
        let case = div()
            .rounded(px(CASE_RADIUS))
            .bg(caps.side)
            .pb(px(CASE_LIP))
            .shadow_lg()
            .child(
                div()
                    .p_4()
                    .rounded(px(CASE_RADIUS))
                    .bg(caps.case)
                    .border_1()
                    .border_color(caps.plate)
                    .child(leds),
            );
        Some(panel::stage(case))
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
