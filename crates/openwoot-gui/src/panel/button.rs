use std::rc::Rc;

use gpui_kit::{
    App, ClickEvent, ElementId, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, RenderOnce, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window,
    base::StyledExt as _,
    component::{ActiveTheme as _, Icon},
    div,
    prelude::FluentBuilder as _,
    px,
};

use super::tip::{Tip, with_tip};
use crate::theme;

const RADIUS: f32 = 6.0;
const ICON_ONLY: f32 = 20.0;
const ICON_WITH_LABEL: f32 = 16.0;
const ICON_GAP: f32 = 8.0;
const DISABLED_OPACITY: f32 = 0.4;
const GHOST_DISABLED_OPACITY: f32 = 0.3;

type Click = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    Secondary,
    Primary,
    Danger,
    Ghost,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Height {
    Tiny,
    Small,
    Large,
}

struct Look {
    background: Hsla,
    hover: Hsla,
    pressed: Hsla,
    foreground: Hsla,
    hover_foreground: Hsla,
    pressed_foreground: Hsla,
}

#[derive(IntoElement)]
pub(crate) struct ActionButton {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<Icon>,
    icon_size: Option<Pixels>,
    variant: Variant,
    height: Height,
    disabled: bool,
    selected: bool,
    tooltip: Option<Tip>,
    on_click: Option<Click>,
    style: StyleRefinement,
}

impl ActionButton {
    pub(crate) fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: Some(label.into()),
            icon: None,
            icon_size: None,
            variant: Variant::Secondary,
            height: Height::Small,
            disabled: false,
            selected: false,
            tooltip: None,
            on_click: None,
            style: StyleRefinement::default(),
        }
    }

    pub(crate) fn icon_size(mut self, size: Pixels) -> Self {
        self.icon_size = Some(size);
        self
    }

    pub(crate) fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub(crate) fn icon_only(id: impl Into<ElementId>, icon: impl Into<Icon>) -> Self {
        Self {
            label: None,
            icon: Some(icon.into()),
            variant: Variant::Ghost,
            ..Self::new(id, SharedString::default())
        }
    }

    pub(crate) fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub(crate) fn primary(mut self) -> Self {
        self.variant = Variant::Primary;
        self
    }

    pub(crate) fn danger(mut self) -> Self {
        self.variant = Variant::Danger;
        self
    }

    pub(crate) fn ghost(mut self) -> Self {
        self.variant = Variant::Ghost;
        self
    }

    pub(crate) fn plain(mut self) -> Self {
        self.variant = Variant::Plain;
        self
    }

    pub(crate) fn filled(mut self) -> Self {
        self.variant = Variant::Secondary;
        self
    }

    pub(crate) fn large(mut self) -> Self {
        self.height = Height::Large;
        self
    }

    pub(crate) fn tiny(mut self) -> Self {
        self.height = Height::Tiny;
        self
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub(crate) fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(Tip::Text(tooltip.into()));
        self
    }

    pub(crate) fn tip(mut self, tip: Tip) -> Self {
        self.tooltip = Some(tip);
        self
    }

    pub(crate) fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for ActionButton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Height {
    const fn pixels(self) -> f32 {
        match self {
            Self::Tiny => 24.0,
            Self::Small => 32.0,
            Self::Large => 40.0,
        }
    }

    const fn padding(self) -> f32 {
        match self {
            Self::Tiny => 8.0,
            Self::Small => 12.0,
            Self::Large => 16.0,
        }
    }

    const fn text(self) -> f32 {
        match self {
            Self::Tiny => 12.0,
            Self::Small => 14.0,
            Self::Large => 16.0,
        }
    }
}

impl RenderOnce for ActionButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Look {
            background,
            hover,
            pressed,
            foreground,
            hover_foreground,
            pressed_foreground,
        } = look(self.variant, self.disabled, self.selected, cx);
        let height = px(self.height.pixels());
        let square = self.label.is_none();
        let icon_size = self
            .icon_size
            .unwrap_or_else(|| px(if square { ICON_ONLY } else { ICON_WITH_LABEL }));
        let disabled_opacity = if matches!(self.variant, Variant::Ghost | Variant::Plain) {
            GHOST_DISABLED_OPACITY
        } else {
            DISABLED_OPACITY
        };
        let key = self.id.clone();
        let element = div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(ICON_GAP))
            .h(height)
            .when(square, |this| this.w(height))
            .when(!square, |this| this.px(px(self.height.padding())))
            .rounded(px(RADIUS))
            .bg(background)
            .text_color(foreground)
            .text_size(px(self.height.text()))
            .font_weight(FontWeight::SEMIBOLD)
            .whitespace_nowrap()
            .when(self.disabled, |this| {
                this.opacity(disabled_opacity).cursor_not_allowed()
            })
            .when(!self.disabled, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.bg(hover).text_color(hover_foreground))
                    .active(move |style| style.bg(pressed).text_color(pressed_foreground))
            })
            .refine_style(&self.style)
            .when_some(self.icon, |this, icon| this.child(icon.size(icon_size)))
            .when_some(self.label, gpui_kit::ParentElement::child)
            .when_some(
                self.on_click.filter(|_| !self.disabled),
                |this, on_click| {
                    this.on_click(move |event, window, cx| on_click(event, window, cx))
                },
            );
        match self.tooltip {
            Some(tip) => with_tip(element, key, tip, window, cx),
            None => element,
        }
    }
}

fn look(variant: Variant, disabled: bool, selected: bool, cx: &App) -> Look {
    let theme = cx.theme();
    match (variant, disabled) {
        (Variant::Secondary | Variant::Primary | Variant::Danger | Variant::Ghost, false)
            if selected =>
        {
            Look {
                background: theme.secondary,
                hover: theme.secondary,
                pressed: theme.secondary,
                foreground: theme.primary,
                hover_foreground: theme.primary,
                pressed_foreground: theme.primary,
            }
        }
        (Variant::Plain, _) if selected => Look {
            background: theme::palette().ghost_active,
            hover: theme::palette().ghost_active,
            pressed: theme::palette().ghost_active,
            foreground: theme.primary,
            hover_foreground: theme.primary,
            pressed_foreground: theme.primary,
        },
        (Variant::Plain, _) => Look {
            background: gpui_kit::transparent_black(),
            hover: theme::palette().ghost_hover,
            pressed: theme::palette().ghost_active,
            foreground: theme.muted_foreground,
            hover_foreground: theme.muted_foreground,
            pressed_foreground: theme.primary,
        },
        (Variant::Ghost, _) => Look {
            background: gpui_kit::transparent_black(),
            hover: theme.secondary,
            pressed: theme.secondary,
            foreground: theme.muted_foreground,
            hover_foreground: theme.group_box_foreground,
            pressed_foreground: theme.primary,
        },
        (_, true) => Look {
            background: theme.accent,
            hover: theme.accent,
            pressed: theme.accent,
            foreground: theme.muted_foreground,
            hover_foreground: theme.muted_foreground,
            pressed_foreground: theme.muted_foreground,
        },
        (Variant::Secondary, false) => Look {
            background: theme.secondary,
            hover: theme.secondary_hover,
            pressed: theme.secondary_active,
            foreground: theme.secondary_foreground,
            hover_foreground: theme.secondary_foreground,
            pressed_foreground: theme.secondary_foreground,
        },
        (Variant::Primary, false) => Look {
            background: theme.primary,
            hover: theme.primary_hover,
            pressed: theme.primary_active,
            foreground: theme.primary_foreground,
            hover_foreground: theme.primary_foreground,
            pressed_foreground: theme.primary_foreground,
        },
        (Variant::Danger, false) => Look {
            background: theme.danger,
            hover: theme.danger_hover,
            pressed: theme.danger_active,
            foreground: theme.danger_foreground,
            hover_foreground: theme.danger_foreground,
            pressed_foreground: theme.danger_foreground,
        },
    }
}
