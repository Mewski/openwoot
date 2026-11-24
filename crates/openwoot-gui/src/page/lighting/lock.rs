use gpui_kit::{
    Context, Div, ParentElement as _, Styled as _, Window,
    component::{Disableable as _, color_picker::ColorPicker, h_flex, switch::Switch},
};
use openwoot_protocol::lighting::{Color, Indicators};

use super::{LightingPage, WHITE, hsla_of, opaque_of};
use crate::panel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lock {
    Caps,
    Scroll,
    Num,
    Layer,
}

impl Lock {
    pub(super) const ALL: [Self; 4] = [Self::Caps, Self::Scroll, Self::Num, Self::Layer];

    pub(super) const fn color(self, indicators: &Indicators) -> Color {
        match self {
            Self::Caps => indicators.caps_lock,
            Self::Scroll => indicators.scroll_lock,
            Self::Num => indicators.num_lock,
            Self::Layer => indicators.layer_lock,
        }
    }

    pub(super) const fn set(self, indicators: &mut Indicators, color: Color) {
        match self {
            Self::Caps => indicators.caps_lock = color,
            Self::Scroll => indicators.scroll_lock = color,
            Self::Num => indicators.num_lock = color,
            Self::Layer => indicators.layer_lock = color,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Caps => "Caps Lock",
            Self::Scroll => "Scroll Lock",
            Self::Num => "Num Lock",
            Self::Layer => "Layer lock",
        }
    }

    const fn detail(self) -> &'static str {
        match self {
            Self::Caps => "Colours the Caps Lock key while Caps Lock is on.",
            Self::Scroll => "Colours the Scroll Lock key while Scroll Lock is on.",
            Self::Num => {
                "Colours the Num Lock key while Num Lock is on, or while it is off when the \
                 keyboard settings invert the Num Lock indicator."
            }
            Self::Layer => "Colours the Fn key while its layer is locked.",
        }
    }

    const fn id(self) -> &'static str {
        match self {
            Self::Caps => "caps-lock-indicator",
            Self::Scroll => "scroll-lock-indicator",
            Self::Num => "num-lock-indicator",
            Self::Layer => "layer-lock-indicator",
        }
    }
}

impl LightingPage {
    pub(super) fn locks_section(&self, cx: &Context<Self>) -> Div {
        let indicators = self.settings.as_ref().map(|settings| settings.indicators);
        let mut section = panel::section(
            "Lock indicators",
            Some(
                "Each indicator paints its key while the lock is on. Without a colour the key \
                 stays dark while the lock is on."
                    .into(),
            ),
            cx,
        );
        for (lock, picker) in &self.locks {
            let lock = *lock;
            let lit = indicators.is_some_and(|indicators| lock.color(&indicators).alpha != 0);
            section = section.child(panel::row(
                lock.label(),
                Some(lock.detail().into()),
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(ColorPicker::new(picker))
                    .child(
                        Switch::new(lock.id())
                            .checked(lit)
                            .disabled(indicators.is_none())
                            .on_change(cx.listener(move |this, lit: &bool, window, cx| {
                                this.light_lock(lock, *lit, window, cx);
                            })),
                    ),
                cx,
            ));
        }
        section
    }

    fn light_lock(&mut self, lock: Lock, lit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let picker = self
            .locks
            .iter()
            .find(|(other, _)| *other == lock)
            .map(|(_, picker)| picker.clone());
        let color = match (lit, &picker) {
            (false, _) => Color::TRANSPARENT,
            (true, None) => WHITE.opaque(),
            (true, Some(picker)) => {
                let color = picker.read(cx).value().map_or(WHITE.opaque(), opaque_of);
                picker.update(cx, |picker, cx| {
                    picker.set_value(hsla_of(color.rgb()), window, cx);
                });
                color
            }
        };
        self.edit_settings(
            |settings| lock.set(&mut settings.indicators, color),
            window,
            cx,
        );
    }
}
