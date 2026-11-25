use gpui_kit::{
    Context, Div, ParentElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, h_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    lightbar::{Entries, Entry},
    lighting::Rgb,
    profile::Slot,
};

use super::{
    Chosen, LightbarPage, Owner, Tab,
    preview::{self, Profile},
};
use crate::{
    color_picker::hsla_of,
    page::{
        Page,
        lighting::{Choice, choices, level_of, wide_slider},
    },
    panel,
    switch::Switch,
    theme,
};

const BAR_HEIGHT: f32 = 90.0;
const BAR_WIDTH: f32 = 809.0;
const LED_HEIGHT: f32 = 54.0;
const LED_GAP: f32 = 4.0;
const BRIGHTNESS_WIDTH: f32 = 368.0;
const ROW_HEIGHT: f32 = 16.0;

impl LightbarPage {
    pub(super) fn preview(&self, window: &Window, cx: &Context<Self>) -> Option<Div> {
        let settings = self.settings.as_ref()?;
        let leds = self.leds(cx);
        let profile = self.preview_profile(cx);
        let mut drawn = Vec::new();
        if settings.use_global
            && let Some(global) = &self.global
        {
            drawn.extend(shown(global, Owner::Global, self.chosen));
        }
        drawn.extend(shown(&settings.entries, Owner::Profile, self.chosen));
        let colors = if let Some(live) = self.live_colors(leds) {
            live
        } else if settings.enabled {
            preview::paint(drawn, leds, profile)
        } else {
            vec![Rgb::default(); usize::from(leds)]
        };
        let caps = theme::keycaps();
        let width = f32::from(panel::content_width(window)).min(BAR_WIDTH);
        let last = colors.len().saturating_sub(1);
        let segments = h_flex()
            .size_full()
            .gap(px(LED_GAP))
            .px_4()
            .items_center()
            .children(colors.into_iter().enumerate().map(|(index, color)| {
                let lit = color != Rgb::default();
                div()
                    .flex_1()
                    .h(px(LED_HEIGHT))
                    .bg(if lit { hsla_of(color) } else { caps.top })
                    .when(index == 0, gpui_kit::Styled::rounded_l_full)
                    .when(index == last, gpui_kit::Styled::rounded_r_full)
                    .when(lit, gpui_kit::Styled::shadow_md)
            }));
        Some(
            div()
                .w(px(width))
                .h(px(BAR_HEIGHT))
                .rounded_full()
                .bg(caps.case_bottom)
                .border_1()
                .border_color(caps.border)
                .shadow_lg()
                .child(segments),
        )
    }

    pub(super) fn brightness_row(&self, cx: &Context<Self>) -> Option<Div> {
        let settings = self.settings.as_ref()?;
        let theme = cx.theme();
        let linked = settings.brightness.is_none();
        let value = self.brightness.read(cx).value().end();
        let tooltip = if linked {
            "Linked with Global Brightness."
        } else {
            "Link with Global Brightness"
        };
        let link = panel::ActionButton::icon_only(
            "lightbar-link",
            Icon::new(if linked {
                Catalog::Link
            } else {
                Catalog::Unlink
            }),
        )
        .filled()
        .text_color(if linked {
            theme.primary
        } else {
            theme.muted_foreground
        })
        .tooltip(tooltip)
        .on_click(cx.listener(|this, _, window, cx| this.toggle_link(window, cx)));
        let slider = div().flex_1().min_w_0().child(wide_slider(
            &self.brightness,
            format!("{value:.0} %"),
            linked,
            cx,
        ));
        Some(
            div().w(px(BRIGHTNESS_WIDTH)).child(panel::field(
                "Brightness",
                h_flex()
                    .w_full()
                    .h(px(ROW_HEIGHT))
                    .gap_2()
                    .items_center()
                    .child(slider)
                    .child(link),
                cx,
            )),
        )
    }

    pub(super) fn heading(&self, cx: &Context<Self>) -> panel::Heading {
        let heading = panel::heading(Page::Lightbar.title())
            .help("The Light Indicator shows effects based on your keyboard's status.");
        let Some(settings) = self.settings.as_ref() else {
            return heading;
        };
        let tabs = choices(
            "lightbar-tabs",
            ["Effect Presets", "Effect Creator"],
            Some(usize::from(self.tab == Tab::Creator)),
            Choice::Tab,
            cx.listener(|this, index: &usize, _, cx| {
                this.tab = if *index == 1 {
                    Tab::Creator
                } else {
                    Tab::Presets
                };
                cx.notify();
            }),
            cx,
        );
        let enabled = Switch::new("lightbar-enabled")
            .checked(settings.enabled)
            .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                let enabled = *enabled;
                this.edit_settings(|settings| settings.enabled = enabled, window, cx);
            }));
        heading.inline(enabled).centre(tabs)
    }

    fn toggle_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let level = level_of(self.brightness.read(cx).value().end());
        self.edit_settings(
            |settings| {
                settings.brightness = match settings.brightness {
                    Some(_) => None,
                    None => Some(level),
                };
            },
            window,
            cx,
        );
        self.sync_controls(window, cx);
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
