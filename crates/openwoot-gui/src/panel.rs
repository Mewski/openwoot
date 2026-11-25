mod button;
mod card;
mod chip;
mod dialog;
mod modal;
mod raised;
mod tip;

use gpui_kit::{
    AnyElement, App, Div, FontWeight, Global, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, RenderOnce, SharedString, Size, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window,
    component::{ActiveTheme as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px, size,
};

pub(crate) use self::{
    button::ActionButton,
    card::{
        description, field, field_with_help, help, info, info_compact, info_titled, label, note,
        row, section, section_with, section_with_help, section_with_inline, text_field, toggle,
        toggle_described, warning,
    },
    chip::{KBD_GAP, kbd, shortcut},
    dialog::{Confirmation, confirm},
    modal::Modal,
    raised::raised,
    tip::{Tip, tip_surface, with_tip, with_tip_right},
};
use crate::{keyboard::KeyboardView, theme};

pub(crate) const RAIL_WIDTH: f32 = 80.0;
pub(crate) const NAVIGATION_WIDTH: f32 = 264.0;
pub(crate) const SHELL_GAP: f32 = 16.0;
pub(crate) const HEADER_HEIGHT: f32 = 80.0;
const COLUMN_WIDTH: f32 = 1196.0;
const COLUMN_INSET: f32 = 16.0;
const COLUMN_GUTTER: f32 = 32.0;
const STAGE_SHARE: f32 = 0.45;
const STAGE_PADDING: f32 = 16.0;
const CONTENT_STAGE_PADDING: f32 = 32.0;
const CAPTION_TOP: f32 = 4.0;
const CAPTION_BOTTOM: f32 = 16.0;
const CAPTION_TEXT: f32 = 14.0;
const CAPTION_LINE: f32 = 16.8;
const SIDE_WIDTH: f32 = 124.0;
const SIDE_GAP: f32 = 16.0;
const SIDE_TOP: f32 = 48.0;
const SIDE_ITEM_GAP: f32 = 8.0;
const BODY_GAP: f32 = 16.0;
const HEADED_TOP: f32 = 24.0;
const HEADED_GAP: f32 = 20.0;
pub(crate) const SCROLL_OVERHANG: f32 = 19.2;
pub(crate) const SCROLL_PADDING: f32 = SCROLL_GUTTER + SCROLLBAR_WIDTH;
const SCROLL_GUTTER: f32 = 9.6;
const SCROLLBAR_WIDTH: f32 = 9.6;
const HEADING_HEIGHT: f32 = 32.0;
const HEADING_TEXT: f32 = 24.0;
const HEADING_LINE: f32 = 28.8;
const HEADING_GAP: f32 = 8.0;
const CENTRED_HEADING_TOP: f32 = 4.0;
const COLUMNS_GAP: f32 = 16.0;
const MIN_ROOM: f32 = 0.0;

#[derive(IntoElement)]
pub(crate) struct Stage {
    content: StageContent,
    side: Option<AnyElement>,
    caption: Option<(SharedString, bool)>,
    fit_content: bool,
    focus: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Frame {
    header: f32,
}

#[derive(IntoElement)]
pub(crate) struct Heading {
    title: SharedString,
    help: Option<SharedString>,
    inline: Option<AnyElement>,
    centre: Option<AnyElement>,
    trailing: Option<AnyElement>,
    standalone: bool,
}

enum StageContent {
    Keyboard(KeyboardView),
    Element(AnyElement),
}

impl Global for Frame {}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Focus(pub(crate) bool);

impl Global for Focus {}

pub(crate) fn focused(cx: &App) -> bool {
    cx.try_global::<Focus>().is_some_and(|focus| focus.0)
}

pub(crate) fn set_focus(on: bool, cx: &mut App) {
    cx.set_global(Focus(on));
}

impl Frame {
    pub(crate) const fn new(header: f32) -> Self {
        Self { header }
    }
}

pub(crate) fn column() -> Div {
    v_flex()
        .w_full()
        .max_w(px(COLUMN_WIDTH))
        .h_full()
        .min_w_0()
        .pt(px(COLUMN_INSET))
        .pb(px(COLUMN_INSET))
        .pl(px(COLUMN_INSET))
        .pr(px(COLUMN_GUTTER))
}

pub(crate) fn content_width(window: &Window) -> Pixels {
    let shell = RAIL_WIDTH + NAVIGATION_WIDTH + 2.0 * SHELL_GAP;
    let column = (f32::from(window.viewport_size().width) - shell).min(COLUMN_WIDTH);
    px((column - COLUMN_INSET - COLUMN_GUTTER).max(MIN_ROOM))
}

pub(crate) fn page() -> Div {
    v_flex().w_full().h_full().min_h_0()
}

pub(crate) fn page_with_stage(
    id: &'static str,
    stage: Stage,
    heading: impl IntoElement,
    body: impl IntoElement,
) -> Div {
    page().child(stage).child(
        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .gap(px(BODY_GAP))
            .child(heading)
            .child(scroll_area(id, body)),
    )
}

pub(crate) fn page_with_stage_only(id: &'static str, stage: Stage, body: impl IntoElement) -> Div {
    page().child(stage).child(
        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(scroll_area(id, body)),
    )
}

pub(crate) fn page_with_heading(
    id: &'static str,
    heading: impl IntoElement,
    body: impl IntoElement,
) -> Div {
    page()
        .pt(px(HEADED_TOP))
        .gap(px(HEADED_GAP))
        .child(heading)
        .child(scroll_area(id, body))
}

pub(crate) fn heading(title: impl Into<SharedString>) -> Heading {
    Heading {
        title: title.into(),
        help: None,
        inline: None,
        centre: None,
        trailing: None,
        standalone: false,
    }
}

pub(crate) fn columns(left: impl IntoElement, right: impl IntoElement) -> Div {
    h_flex()
        .w_full()
        .items_start()
        .gap(px(COLUMNS_GAP))
        .child(v_flex().flex_1().min_w_0().gap(px(COLUMNS_GAP)).child(left))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(COLUMNS_GAP))
                .child(right),
        )
}

impl Stage {
    pub(crate) fn new(keyboard: KeyboardView) -> Self {
        Self {
            content: StageContent::Keyboard(keyboard),
            side: None,
            caption: None,
            fit_content: false,
            focus: false,
        }
    }

    pub(crate) fn element(content: impl IntoElement) -> Self {
        Self {
            content: StageContent::Element(content.into_any_element()),
            side: None,
            caption: None,
            fit_content: false,
            focus: false,
        }
    }

    pub(crate) fn fit_content(mut self) -> Self {
        self.fit_content = true;
        self
    }

    pub(crate) fn focus(mut self, focus: bool) -> Self {
        self.focus = focus;
        self
    }

    pub(crate) fn side(mut self, side: impl IntoElement) -> Self {
        self.side = Some(side.into_any_element());
        self
    }

    pub(crate) fn caption(mut self, text: impl Into<SharedString>, highlighted: bool) -> Self {
        self.caption = Some((text.into(), highlighted));
        self
    }

    fn room(&self, window: &Window, height: f32) -> Size<Pixels> {
        let side = if self.side.is_some() {
            SIDE_WIDTH + SIDE_GAP
        } else {
            0.0
        };
        let caption = if self.caption.is_some() {
            CAPTION_TOP + CAPTION_LINE + CAPTION_BOTTOM
        } else {
            0.0
        };
        let width = f32::from(content_width(window)) - 2.0 * STAGE_PADDING - side;
        let height = height - 2.0 * STAGE_PADDING - caption;
        size(px(width.max(MIN_ROOM)), px(height.max(MIN_ROOM)))
    }
}

impl RenderOnce for Stage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let header = cx
            .try_global::<Frame>()
            .map_or(HEADER_HEIGHT, |frame| frame.header);
        let height = stage_height(window, header);
        let room = self.room(window, height);
        let content = match self.content {
            StageContent::Keyboard(keyboard) if self.focus => {
                keyboard.fit(room).raise_selection().into_any_element()
            }
            StageContent::Keyboard(keyboard) => keyboard.fit(room).into_any_element(),
            StageContent::Element(element) => element,
        };
        let fit_content = self.fit_content;
        let centred = h_flex()
            .flex_1()
            .min_w_0()
            .when(!fit_content, gpui_kit::Styled::h_full)
            .justify_center()
            .items_center()
            .child(content);
        let body = h_flex()
            .w_full()
            .when(!fit_content, gpui_kit::Styled::h_full)
            .items_start()
            .when_some(self.side, |this, side| {
                this.child(
                    v_flex()
                        .flex_none()
                        .w(px(SIDE_WIDTH))
                        .mr(px(SIDE_GAP))
                        .pt(px(SIDE_TOP))
                        .gap(px(SIDE_ITEM_GAP))
                        .child(side),
                )
            })
            .child(centred);
        let theme = cx.theme();
        let stage = v_flex().flex_none().w_full();
        let stage = if fit_content {
            stage.py(px(CONTENT_STAGE_PADDING)).child(body)
        } else {
            stage.h(px(height)).child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .p(px(STAGE_PADDING))
                    .child(body),
            )
        };
        stage.when_some(self.caption, |this, (text, highlighted)| {
            this.child(
                div()
                    .w_full()
                    .pt(px(CAPTION_TOP))
                    .pb(px(CAPTION_BOTTOM))
                    .flex()
                    .justify_center()
                    .text_size(px(CAPTION_TEXT))
                    .line_height(px(CAPTION_LINE))
                    .font_weight(FontWeight::BOLD)
                    .text_color(if highlighted {
                        theme.primary
                    } else {
                        theme.sidebar_foreground
                    })
                    .child(SharedString::from(text.to_uppercase())),
            )
        })
    }
}

impl Heading {
    pub(crate) fn help(mut self, help: impl Into<SharedString>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub(crate) fn inline(mut self, inline: impl IntoElement) -> Self {
        self.inline = Some(inline.into_any_element());
        self
    }

    pub(crate) fn centre(mut self, centre: impl IntoElement) -> Self {
        self.centre = Some(centre.into_any_element());
        self
    }

    pub(crate) fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    pub(crate) fn standalone(mut self) -> Self {
        self.standalone = true;
        self
    }
}

impl RenderOnce for Heading {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let help = self.help.map(|text| help(&self.title, text, cx));
        let centred = self.centre.is_some();
        div()
            .relative()
            .flex_none()
            .w_full()
            .when(centred, |this| {
                this.min_h(px(HEADING_HEIGHT + CENTRED_HEADING_TOP))
                    .pt(px(CENTRED_HEADING_TOP))
                    .mb(px(-CENTRED_HEADING_TOP))
            })
            .child(
                h_flex()
                    .w_full()
                    .when(centred, |this| this.min_h(px(HEADING_HEIGHT)))
                    .items_end()
                    .justify_between()
                    .gap(px(HEADING_GAP))
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap(px(HEADING_GAP))
                            .child(
                                div()
                                    .text_size(px(HEADING_TEXT))
                                    .line_height(px(HEADING_LINE))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if self.standalone {
                                        cx.theme().foreground
                                    } else {
                                        theme::palette().heading
                                    })
                                    .child(self.title.clone()),
                            )
                            .children(help)
                            .children(self.inline),
                    )
                    .when_some(self.trailing, |this, trailing| {
                        this.child(h_flex().flex_none().gap(px(HEADING_GAP)).child(trailing))
                    }),
            )
            .when_some(self.centre, |this, centre| {
                this.child(
                    h_flex()
                        .absolute()
                        .inset_0()
                        .justify_center()
                        .items_center()
                        .child(centre),
                )
            })
    }
}

pub(crate) fn scroll_area(id: &'static str, body: impl IntoElement) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .min_h_0()
        .mr(px(-SCROLL_OVERHANG))
        .pr(px(SCROLL_PADDING))
        .overflow_y_scroll()
        .child(body)
}

fn stage_height(window: &Window, header: f32) -> f32 {
    let page = f32::from(window.viewport_size().height) - 2.0 * COLUMN_INSET - header;
    (page * STAGE_SHARE).max(MIN_ROOM)
}
