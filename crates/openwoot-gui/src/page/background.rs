use gpui_kit::{
    AnyElement, App, Context, Div, Entity, FontWeight, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, h_flex, v_flex},
    div, px,
};

use super::{
    Page,
    settings::{Tone, body_text, callout, entry, sheet},
};
use crate::{background::Background, store::Store, switch::Switch};

const DESCRIPTION: &str = "The Background Service handles automatic profile switching, which changes your keyboard profile based on the active app.";
const TAG_HEIGHT: f32 = 20.0;
const TAG_PADDING_X: f32 = 4.0;
const TAG_RADIUS: f32 = 6.0;
const TAG_TEXT: f32 = 12.0;
const SOURCE_PADDING: f32 = 12.0;
const SOURCE_GAP: f32 = 16.0;
const SOURCE_TEXT_GAP: f32 = 4.0;
const SOURCE_RADIUS: f32 = 8.0;
const DISC: f32 = 36.0;
const DISC_ICON: f32 = 20.0;
const BODY_TEXT: f32 = 16.0;
const TITLE_LINE: f32 = 19.2;
const BODY_LINE: f32 = 22.4;
const STATUS_GAP: f32 = 16.0;

pub(crate) struct BackgroundPage {
    background: Option<Entity<Background>>,
    _subscriptions: Vec<Subscription>,
}

impl BackgroundPage {
    pub(crate) fn new(store: &Entity<Store>, cx: &mut Context<Self>) -> Self {
        let background = Background::global(cx);
        let mut subscriptions = vec![cx.observe(store, |_, _, cx| cx.notify())];
        subscriptions.extend(
            background
                .as_ref()
                .map(|background| cx.observe(background, |_, _, cx| cx.notify())),
        );
        Self {
            background,
            _subscriptions: subscriptions,
        }
    }

    fn status(running: bool, cx: &App) -> Div {
        v_flex()
            .w_full()
            .items_start()
            .gap(px(STATUS_GAP))
            .child(tag(running, cx))
            .child(body_text(DESCRIPTION, cx))
    }

    fn sources(&self, running: bool, cx: &Context<Self>) -> Div {
        let background = self.background.clone();
        let switch = Switch::new("background-apps")
            .checked(running)
            .disabled(background.is_none())
            .on_change(move |on, _, cx| {
                if let Some(background) = &background {
                    background.update(cx, |background, cx| background.set_running(*on, cx));
                }
            });
        entry(
            "Data sources",
            Some("Choose which data can be accessed by Background Service.".into()),
            None,
            cx,
        )
        .child(source(
            Catalog::Monitor,
            "Games and apps",
            "Automatic profile switching based on active application",
            switch.into_any_element(),
            cx,
        ))
    }

    fn start_with_computer(&self, on: bool, cx: &Context<Self>) -> Div {
        let background = self.background.clone();
        let switch = Switch::new("background-start")
            .checked(on)
            .disabled(background.is_none())
            .on_change(move |on, _, cx| {
                if let Some(background) = &background {
                    background.update(cx, |background, cx| {
                        background.set_start_with_computer(*on, cx);
                    });
                }
            });
        entry(
            "Start with computer",
            Some(
                "Launch the Background Service automatically when your computer starts up.".into(),
            ),
            Some(switch.into_any_element()),
            cx,
        )
    }
}

impl Render for BackgroundPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (running, starts, problem) =
            self.background
                .as_ref()
                .map_or((false, false, None), |background| {
                    let background = background.read(cx);
                    (
                        background.is_running(),
                        background.starts_with_computer(),
                        background.problem().cloned(),
                    )
                });
        let mut sections = vec![
            Self::status(running, cx),
            self.sources(running, cx),
            self.start_with_computer(starts, cx),
        ];
        sections.extend(problem.map(|problem| callout(Tone::Danger, problem, cx)));
        sheet("background-sheet", Page::Background.title(), sections, cx)
    }
}

fn tag(running: bool, cx: &App) -> Div {
    let theme = cx.theme();
    let (label, background) = if running {
        ("CONNECTED", theme.success)
    } else {
        ("DISCONNECTED", theme.danger)
    };
    div()
        .flex_none()
        .h(px(TAG_HEIGHT))
        .px(px(TAG_PADDING_X))
        .flex()
        .items_center()
        .rounded(px(TAG_RADIUS))
        .bg(background)
        .text_size(px(TAG_TEXT))
        .font_weight(FontWeight::BOLD)
        .text_color(theme.primary_foreground)
        .child(label)
}

fn source(
    icon: Catalog,
    title: &'static str,
    description: &'static str,
    control: AnyElement,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .p(px(SOURCE_PADDING))
        .gap(px(SOURCE_GAP))
        .justify_between()
        .rounded(px(SOURCE_RADIUS))
        .bg(theme.tab_active)
        .child(
            h_flex()
                .min_w_0()
                .gap(px(SOURCE_PADDING))
                .child(
                    div()
                        .flex_none()
                        .size(px(DISC))
                        .rounded_full()
                        .bg(theme.secondary)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Icon::new(icon)
                                .size(px(DISC_ICON))
                                .text_color(theme.foreground),
                        ),
                )
                .child(
                    v_flex()
                        .min_w_0()
                        .gap(px(SOURCE_TEXT_GAP))
                        .child(
                            div()
                                .text_size(px(BODY_TEXT))
                                .line_height(px(TITLE_LINE))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground)
                                .child(SharedString::from(title)),
                        )
                        .child(
                            div()
                                .text_size(px(BODY_TEXT))
                                .line_height(px(BODY_LINE))
                                .text_color(theme.group_box_foreground)
                                .child(SharedString::from(description)),
                        ),
                ),
        )
        .child(div().flex_none().child(control))
}
