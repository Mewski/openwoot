use gpui_kit::{
    App, Axis, BoxShadow, Entity, IntoElement, ParentElement as _, RenderOnce, Styled as _, Window,
    base::{Slider as Rail, SliderIndicator, SliderThumb, SliderTrack},
    component::{ActiveTheme as _, slider::SliderState},
    div, hsla, point,
    prelude::FluentBuilder as _,
    px, relative,
};

const TRACK: f32 = 4.0;
const THUMB: f32 = 16.0;
const RAIL: f32 = 14.0;
const DISABLED_OPACITY: f32 = 0.6;

#[derive(IntoElement)]
pub(crate) struct Slider {
    state: Entity<SliderState>,
    disabled: bool,
}

impl Slider {
    pub(crate) fn new(state: &Entity<SliderState>) -> Self {
        Self {
            state: state.clone(),
            disabled: false,
        }
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let percentage = self.state.read(cx).percentage();
        let range = self.state.read(cx).value().is_range();
        let (fill, ring) = if self.disabled {
            (theme.sidebar_foreground, theme.muted_foreground)
        } else {
            (theme.primary, theme.primary)
        };
        let thumb = |fraction: f32, start: bool| {
            SliderThumb::new(&self.state)
                .axis(Axis::Horizontal)
                .start(start)
                .disabled(self.disabled)
                .absolute()
                .top(px((TRACK - THUMB) / 2.0))
                .left(relative(fraction))
                .ml(px(-THUMB / 2.0))
                .size(px(THUMB))
                .rounded_full()
                .bg(theme.background)
                .border_1()
                .border_color(ring)
                .shadow(shadow())
        };
        let indicator = SliderIndicator::new(&self.state)
            .relative()
            .w_full()
            .h(px(TRACK))
            .rounded_full()
            .bg(theme.secondary_active)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(percentage.start))
                    .right(relative(1.0 - percentage.end))
                    .rounded_full()
                    .bg(fill),
            )
            .when(range, |this| this.child(thumb(percentage.start, true)))
            .child(thumb(percentage.end, false));
        div()
            .w_full()
            .when(self.disabled, |this| {
                this.opacity(DISABLED_OPACITY).cursor_not_allowed()
            })
            .when(!self.disabled, gpui_kit::Styled::cursor_pointer)
            .child(
                Rail::new(&self.state)
                    .axis(Axis::Horizontal)
                    .disabled(self.disabled)
                    .flex()
                    .w_full()
                    .child(
                        SliderTrack::new(&self.state)
                            .axis(Axis::Horizontal)
                            .disabled(self.disabled)
                            .flex()
                            .items_center()
                            .h(px(RAIL))
                            .w_full()
                            .child(indicator),
                    ),
            )
    }
}

fn shadow() -> Vec<BoxShadow> {
    [(0.1, 3.0), (0.06, 2.0)]
        .into_iter()
        .map(|(alpha, blur)| BoxShadow {
            color: hsla(0.0, 0.0, 0.0, alpha),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(blur),
            spread_radius: px(0.0),
            inset: false,
        })
        .collect()
}
