use std::rc::Rc;

use gpui_kit::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName,
    component::{ActiveTheme as _, Icon},
    div,
    prelude::FluentBuilder as _,
    px,
};

const TRACK_WIDTH: f32 = 44.0;
const TRACK_HEIGHT: f32 = 24.0;
const THUMB: f32 = 18.0;
const INSET: f32 = 3.0;
const MARK: f32 = 12.0;

type Change = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(crate) struct Switch {
    id: ElementId,
    checked: bool,
    mixed: bool,
    disabled: bool,
    on_change: Option<Change>,
}

impl Switch {
    pub(crate) fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            mixed: false,
            disabled: false,
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
        let theme = cx.theme();
        let state = match (self.mixed, self.checked) {
            (true, _) => Shown::Mixed,
            (false, true) => Shown::On,
            (false, false) => Shown::Off,
        };
        let (track, mark, mark_color) = match state {
            Shown::On => (
                theme.primary,
                Some(IconName::Check),
                theme.primary_foreground,
            ),
            Shown::Off => (theme.switch, Some(IconName::Close), theme.muted_foreground),
            Shown::Mixed => (theme.switch, None, theme.muted_foreground),
        };
        let mark_left = match state {
            Shown::On => px(INSET + (THUMB - MARK) / 2.0),
            Shown::Off | Shown::Mixed => px(TRACK_WIDTH - INSET - THUMB + (THUMB - MARK) / 2.0),
        };
        let thumb_left = match state {
            Shown::On => px(TRACK_WIDTH - INSET - THUMB),
            Shown::Off => px(INSET),
            Shown::Mixed => px((TRACK_WIDTH - THUMB) / 2.0),
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
            .when(self.disabled, |this| this.opacity(0.45))
            .when(!self.disabled, gpui_kit::Styled::cursor_pointer)
            .when_some(mark, |this, mark| {
                this.child(
                    div()
                        .absolute()
                        .top(px((TRACK_HEIGHT - MARK) / 2.0))
                        .left(mark_left)
                        .size(px(MARK))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(mark).size(px(MARK)).text_color(mark_color)),
                )
            })
            .child(
                div()
                    .absolute()
                    .top(px(INSET))
                    .left(thumb_left)
                    .size(px(THUMB))
                    .rounded_full()
                    .bg(theme.background),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    On,
    Off,
    Mixed,
}
