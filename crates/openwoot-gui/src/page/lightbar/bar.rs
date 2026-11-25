use gpui_kit::{
    Context, Div, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, Sizable as _, h_flex, slider::Slider, tooltip::Tooltip, v_flex,
    },
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
        lighting::{Choice, choices, level_of},
    },
    panel,
    switch::Switch,
    theme,
};

const BAR_HEIGHT: f32 = 56.0;
const BAR_WIDTH: f32 = 760.0;
const LED_GAP: f32 = 6.0;
const BRIGHTNESS_WIDTH: f32 = 400.0;
const LINK_SIZE: f32 = 32.0;

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
        let colors = if settings.enabled {
            preview::paint(drawn, leds, profile)
        } else {
            vec![Rgb::default(); usize::from(leds)]
        };
        let caps = theme::keycaps();
        let width = (f32::from(panel::content_width(window)) * 0.8).min(BAR_WIDTH);
        let segments = h_flex()
            .size_full()
            .gap(px(LED_GAP))
            .px_4()
            .py_2()
            .rounded_full()
            .bg(caps.case)
            .children(colors.into_iter().map(|color| {
                let lit = color != Rgb::default();
                div()
                    .flex_1()
                    .h_full()
                    .rounded(px(8.0))
                    .bg(if lit { hsla_of(color) } else { caps.top })
                    .when(lit, gpui_kit::Styled::shadow_md)
            }));
        Some(panel::stage(
            div()
                .w(px(width))
                .h(px(BAR_HEIGHT))
                .p_1()
                .rounded_full()
                .bg(caps.side)
                .border_1()
                .border_color(caps.plate)
                .shadow_lg()
                .child(segments),
        ))
    }

    pub(super) fn brightness_row(&self, cx: &Context<Self>) -> Option<Div> {
        let settings = self.settings.as_ref()?;
        let theme = cx.theme();
        let linked = settings.brightness.is_none();
        let value = self.brightness.read(cx).value().end();
        let tooltip = if linked {
            "Linked with the global brightness; click to set its own"
        } else {
            "Link with the global brightness"
        };
        Some(panel::stage(
            v_flex()
                .w(px(BRIGHTNESS_WIDTH))
                .gap_2()
                .child(panel::label("Brightness", cx))
                .child(
                    h_flex()
                        .w_full()
                        .gap_3()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .when(linked, |this| this.opacity(0.45))
                                .child(Slider::new(&self.brightness).disabled(linked)),
                        )
                        .child(
                            div()
                                .w(px(48.0))
                                .text_sm()
                                .text_color(if linked {
                                    theme.muted_foreground
                                } else {
                                    theme.foreground
                                })
                                .child(format!("{value:.0} %")),
                        )
                        .child(
                            div()
                                .id("lightbar-link")
                                .size(px(LINK_SIZE))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(6.0))
                                .cursor_pointer()
                                .bg(if linked {
                                    theme.secondary
                                } else {
                                    theme.transparent
                                })
                                .border_1()
                                .border_color(theme.border)
                                .text_color(if linked {
                                    theme.primary
                                } else {
                                    theme.muted_foreground
                                })
                                .hover(|style| style.bg(theme.secondary_hover))
                                .child(
                                    Icon::new(if linked {
                                        Catalog::Link
                                    } else {
                                        Catalog::Unlink
                                    })
                                    .small(),
                                )
                                .tooltip(move |window, cx| Tooltip::new(tooltip).build(window, cx))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.toggle_link(window, cx)),
                                ),
                        ),
                ),
        ))
    }

    pub(super) fn heading(&self, cx: &Context<Self>) -> Option<Div> {
        let settings = self.settings.as_ref()?;
        let tabs = choices(
            "lightbar-tabs",
            ["Effect presets", "Effect creator"],
            Some(usize::from(self.tab == Tab::Creator)),
            Choice::Outlined,
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
        Some(panel::heading(
            Page::Lightbar.title(),
            Some(SharedString::from(
                "Effects on the LED strip: the active profile's own, plus the global ones",
            )),
            h_flex()
                .gap_6()
                .items_center()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Enabled"),
                        )
                        .child(
                            Switch::new("lightbar-enabled")
                                .checked(settings.enabled)
                                .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                                    let enabled = *enabled;
                                    this.edit_settings(
                                        |settings| settings.enabled = enabled,
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                )
                .child(tabs),
            cx,
        ))
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
