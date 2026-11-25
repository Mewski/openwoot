use std::rc::Rc;

use gpui_kit::{
    App, Div, FontWeight, Hsla, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
    component::{ActiveTheme as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::device::Layout;

const UNIT: f32 = 20.0;
const INSET: f32 = 1.5;
const WIDTH_UNITS: f32 = 4.5;
const ROWS: f32 = 4.0;
const GLYPH_SIZE: f32 = 10.0;
const CARD_GAP: f32 = 8.0;
const CARD_INNER_GAP: f32 = 12.0;
const CARD_TOP: f32 = 12.0;
const CARD_SIDE: f32 = 4.0;
const LABEL_SIZE: f32 = 16.0;
const LABEL_LINE: f32 = 19.2;

pub(in crate::page) const fn name(layout: Layout) -> &'static str {
    match layout {
        Layout::Ansi => "ANSI",
        Layout::Iso => "ISO",
        Layout::Jis => "JIS",
    }
}

pub(super) fn cards(
    layouts: &[Layout],
    selected: Option<Layout>,
    on_choose: impl Fn(&Layout, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let on_choose = Rc::new(on_choose);
    h_flex()
        .w_full()
        .items_stretch()
        .gap(px(CARD_GAP))
        .children(layouts.iter().enumerate().map(|(index, layout)| {
            let layout = *layout;
            let chosen = selected == Some(layout);
            let on_choose = on_choose.clone();
            let hover_background = theme.secondary;
            let hover_border = theme.secondary_hover;
            v_flex()
                .id(("layout", index))
                .flex_1()
                .items_center()
                .gap(px(CARD_INNER_GAP))
                .pt(px(CARD_TOP))
                .px(px(CARD_SIDE))
                .pb(px(CARD_SIDE))
                .rounded(theme.radius)
                .border_1()
                .border_color(if chosen {
                    theme.primary
                } else {
                    theme.transparent
                })
                .bg(if chosen {
                    theme.secondary
                } else {
                    theme.accent
                })
                .when(!chosen, |this| {
                    this.cursor_pointer()
                        .hover(move |style| style.bg(hover_background).border_color(hover_border))
                })
                .child(drawing(layout, cx))
                .child(
                    div()
                        .text_size(px(LABEL_SIZE))
                        .line_height(px(LABEL_LINE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(name(layout)),
                )
                .on_click(move |_, window, cx| {
                    if !chosen {
                        on_choose(&layout, window, cx);
                    }
                })
        }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Plain,
    Backspace,
    Enter,
    Shift,
}

#[derive(Debug, Clone, Copy)]
struct Cap {
    x: f32,
    row: f32,
    width: f32,
    mark: Mark,
}

const fn cap(x: f32, row: f32, width: f32) -> Cap {
    Cap {
        x,
        row,
        width,
        mark: Mark::Plain,
    }
}

const fn marked(x: f32, row: f32, width: f32, mark: Mark) -> Cap {
    Cap {
        x,
        row,
        width,
        mark,
    }
}

const ANSI: [Cap; 12] = [
    cap(0.5, 0.0, 1.0),
    cap(1.5, 0.0, 1.0),
    marked(2.5, 0.0, 2.0, Mark::Backspace),
    cap(0.0, 1.0, 1.0),
    cap(1.0, 1.0, 1.0),
    cap(2.0, 1.0, 1.0),
    cap(3.0, 1.0, 1.5),
    cap(0.25, 2.0, 1.0),
    cap(1.25, 2.0, 1.0),
    marked(2.25, 2.0, 2.25, Mark::Enter),
    cap(0.75, 3.0, 1.0),
    marked(1.75, 3.0, 2.75, Mark::Shift),
];

const ISO: [Cap; 11] = [
    cap(0.5, 0.0, 1.0),
    cap(1.5, 0.0, 1.0),
    marked(2.5, 0.0, 2.0, Mark::Backspace),
    cap(0.0, 1.0, 1.0),
    cap(1.0, 1.0, 1.0),
    cap(2.0, 1.0, 1.0),
    cap(0.25, 2.0, 1.0),
    cap(1.25, 2.0, 1.0),
    cap(2.25, 2.0, 1.0),
    cap(0.75, 3.0, 1.0),
    marked(1.75, 3.0, 2.75, Mark::Shift),
];

const JIS: [Cap; 13] = [
    cap(0.5, 0.0, 1.0),
    cap(1.5, 0.0, 1.0),
    cap(2.5, 0.0, 1.0),
    marked(3.5, 0.0, 1.0, Mark::Backspace),
    cap(0.0, 1.0, 1.0),
    cap(1.0, 1.0, 1.0),
    cap(2.0, 1.0, 1.0),
    cap(0.25, 2.0, 1.0),
    cap(1.25, 2.0, 1.0),
    cap(2.25, 2.0, 1.0),
    cap(0.75, 3.0, 1.0),
    cap(1.75, 3.0, 1.0),
    marked(2.75, 3.0, 1.75, Mark::Shift),
];

fn drawing(layout: Layout, cx: &App) -> Div {
    let theme = cx.theme();
    let (caps, tall_enter): (&[Cap], bool) = match layout {
        Layout::Ansi => (&ANSI, false),
        Layout::Iso => (&ISO, true),
        Layout::Jis => (&JIS, true),
    };
    let outline = theme.muted_foreground;
    let accent = theme.primary;
    div()
        .relative()
        .flex_none()
        .w(px(WIDTH_UNITS * UNIT))
        .h(px(ROWS * UNIT))
        .children(caps.iter().map(|cap| keycap(*cap, outline, accent)))
        .when(tall_enter, |this| this.children(tall_enter_outline(accent)))
}

fn keycap(cap: Cap, outline: Hsla, accent: Hsla) -> Div {
    let color = match cap.mark {
        Mark::Backspace | Mark::Enter => accent,
        Mark::Plain | Mark::Shift => outline,
    };
    let glyph = match cap.mark {
        Mark::Plain => None,
        Mark::Backspace => Some("←"),
        Mark::Enter => Some("↵"),
        Mark::Shift => Some("⇧"),
    };
    div()
        .absolute()
        .left(px(cap.x * UNIT + INSET))
        .top(px(cap.row * UNIT + INSET))
        .w(px(cap.width * UNIT - 2.0 * INSET))
        .h(px(UNIT - 2.0 * INSET))
        .rounded(px(3.0))
        .border_1()
        .border_color(color)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(GLYPH_SIZE))
        .text_color(color)
        .children(glyph.map(SharedString::from))
}

fn tall_enter_outline(accent: Hsla) -> [Div; 7] {
    let left = 3.0 * UNIT + INSET;
    let step = 3.25 * UNIT + INSET;
    let right = WIDTH_UNITS * UNIT - INSET;
    let top = UNIT + INSET;
    let middle = 2.0 * UNIT - INSET;
    let bottom = 3.0 * UNIT - INSET;
    let line = |x: f32, y: f32, width: f32, height: f32| {
        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(width))
            .h(px(height))
            .bg(accent)
    };
    [
        line(left, top, right - left, 1.0),
        line(right - 1.0, top, 1.0, bottom - top),
        line(step, bottom - 1.0, right - step, 1.0),
        line(step, middle - 1.0, 1.0, bottom - middle + 1.0),
        line(left, middle - 1.0, step - left, 1.0),
        line(left, top, 1.0, middle - top),
        div()
            .absolute()
            .left(px(step))
            .top(px(middle))
            .w(px(right - step))
            .h(px(bottom - middle))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(GLYPH_SIZE))
            .text_color(accent)
            .child("↵"),
    ]
}
