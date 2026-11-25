use std::rc::Rc;

use gpui_kit::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName,
    component::{ActiveTheme as _, Icon},
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::theme;

const TRACK_WIDTH: f32 = 48.0;
const TRACK_HEIGHT: f32 = 24.0;
const THUMB: f32 = 16.0;
const INSET: f32 = 4.0;
const MARK_BOX: f32 = 20.0;
const CROSS: f32 = 13.0;
const CHECK: f32 = 15.0;
const OFF_HOVER_ALPHA: f32 = 0.8;
const DISABLED_OPACITY: f32 = 0.4;

type Change = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(crate) struct Switch {
    id: ElementId,
    checked: bool,
    mixed: bool,
    disabled: bool,
    marks: Marks,
    on_change: Option<Change>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    On,
    Off,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marks {
    Shown,
    Hidden,
}

struct Look {
    track: Hsla,
    hover: Hsla,
    thumb: Hsla,
    mark: Hsla,
}

impl Switch {
    pub(crate) fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            mixed: false,
            disabled: false,
            marks: Marks::Shown,
            on_change: None,
        }
    }

    pub(crate) fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub(crate) fn mixed(mut self, mixed: bool) -> Self {
        self.mixed = mixed;
        self
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub(crate) fn unmarked(mut self) -> Self {
        self.marks = Marks::Hidden;
        self
    }

    pub(crate) fn on_change(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = match (self.mixed, self.checked) {
            (true, _) => Shown::Mixed,
            (false, true) => Shown::On,
            (false, false) => Shown::Off,
        };
        let Look {
            track,
            hover,
            thumb,
            mark,
        } = look(state, self.disabled, cx);
        let (glyph, glyph_size, mark_left) = match state {
            Shown::On => (Some(IconName::Check), CHECK, INSET),
            Shown::Off => (Some(IconName::Close), CROSS, TRACK_WIDTH / 2.0),
            Shown::Mixed => (None, CROSS, INSET),
        };
        let glyph = glyph.filter(|_| self.marks == Marks::Shown);
        let thumb_left = match state {
            Shown::On => TRACK_WIDTH - INSET - THUMB,
            Shown::Off => INSET,
            Shown::Mixed => (TRACK_WIDTH - THUMB) / 2.0,
        };
        let next = state != Shown::On;
        div()
            .id(self.id)
            .relative()
            .flex_none()
            .w(px(TRACK_WIDTH))
            .h(px(TRACK_HEIGHT))
            .rounded_full()
            .bg(track)
            .when(self.disabled, |this| {
                this.opacity(DISABLED_OPACITY).cursor_not_allowed()
            })
            .when(!self.disabled, |this| {
                this.cursor_pointer().hover(move |style| style.bg(hover))
            })
            .when_some(glyph, |this, glyph| {
                this.child(
                    div()
                        .absolute()
                        .top(px((TRACK_HEIGHT - MARK_BOX) / 2.0))
                        .left(px(mark_left))
                        .size(px(MARK_BOX))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(glyph).size(px(glyph_size)).text_color(mark)),
                )
            })
            .child(
                div()
                    .absolute()
                    .top(px((TRACK_HEIGHT - THUMB) / 2.0))
                    .left(px(thumb_left))
                    .size(px(THUMB))
                    .rounded_full()
                    .bg(thumb),
            )
            .when_some(
                self.on_change.filter(|_| !self.disabled),
                |this, on_change| {
                    this.on_click(move |_, window, cx| {
                        on_change(&next, window, cx);
                    })
                },
            )
    }
}

fn look(state: Shown, disabled: bool, cx: &App) -> Look {
    let theme = cx.theme();
    let off = theme.secondary_active;
    match (state, disabled) {
        (_, true) => Look {
            track: theme.accent,
            hover: theme.accent,
            thumb: theme::palette().disabled_thumb,
            mark: theme.muted_foreground,
        },
        (Shown::On, false) => Look {
            track: theme.primary,
            hover: theme.primary_hover,
            thumb: theme.background,
            mark: theme.primary_foreground,
        },
        (Shown::Off | Shown::Mixed, false) => Look {
            track: off,
            hover: Hsla {
                a: OFF_HOVER_ALPHA,
                ..off
            },
            thumb: theme.background,
            mark: theme.background,
        },
    }
}
