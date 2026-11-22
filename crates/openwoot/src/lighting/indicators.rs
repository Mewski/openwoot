use super::{
    color::opaque,
    config::{Color, Indicators},
    leds::Key,
    overlay::{Overlay, OverlayId},
};

const WHITE: Color = Color {
    red: 255,
    green: 255,
    blue: 255,
    alpha: 255,
};

#[expect(
    clippy::struct_excessive_bools,
    reason = "the four lock states are independent host and layer states"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LockState {
    pub caps_lock: bool,
    pub scroll_lock: bool,
    pub num_lock: bool,
    pub layer_locked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Indicator {
    CapsLock,
    ScrollLock,
    NumLock,
    LayerLock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LockIndicators {
    colors: Indicators,
    keys: [Option<Key>; 4],
    locks: LockState,
    num_lock_inverted: bool,
}

impl Indicator {
    const ALL: [Self; 4] = [
        Self::LayerLock,
        Self::CapsLock,
        Self::ScrollLock,
        Self::NumLock,
    ];

    const fn overlay(self) -> OverlayId {
        match self {
            Self::CapsLock => OverlayId::CapsLock,
            Self::ScrollLock => OverlayId::ScrollLock,
            Self::NumLock => OverlayId::NumLock,
            Self::LayerLock => OverlayId::LayerLock,
        }
    }

    const fn slot(self) -> usize {
        match self {
            Self::CapsLock => 0,
            Self::ScrollLock => 1,
            Self::NumLock => 2,
            Self::LayerLock => 3,
        }
    }
}

impl LockIndicators {
    pub(super) const fn new() -> Self {
        Self {
            colors: Indicators {
                caps_lock: WHITE,
                scroll_lock: WHITE,
                num_lock: WHITE,
                layer_lock: WHITE,
            },
            keys: [None; 4],
            locks: LockState {
                caps_lock: false,
                scroll_lock: false,
                num_lock: false,
                layer_locked: false,
            },
            num_lock_inverted: true,
        }
    }

    pub(super) const fn set_colors(&mut self, colors: Indicators) {
        self.colors = colors;
    }

    pub(super) const fn set_locks(&mut self, locks: LockState) {
        self.locks = locks;
    }

    pub(super) const fn set_num_lock_inverted(&mut self, inverted: bool) {
        self.num_lock_inverted = inverted;
    }

    pub(super) fn set_key(&mut self, indicator: Indicator, key: Option<Key>) {
        if let Some(slot) = self.keys.get_mut(indicator.slot()) {
            *slot = key;
        }
    }

    pub(super) fn overlays(&self, host_control: bool) -> [(OverlayId, Option<Overlay>); 4] {
        Indicator::ALL.map(|indicator| (indicator.overlay(), self.overlay(indicator, host_control)))
    }

    fn overlay(&self, indicator: Indicator, host_control: bool) -> Option<Overlay> {
        let locks = self.locks;
        let on = match indicator {
            Indicator::LayerLock => locks.layer_locked,
            Indicator::CapsLock => locks.caps_lock,
            Indicator::ScrollLock => locks.scroll_lock,
            Indicator::NumLock => locks.num_lock != self.num_lock_inverted,
        };
        let hidden = indicator != Indicator::LayerLock && host_control;
        let key = self
            .keys
            .get(indicator.slot())
            .copied()
            .flatten()
            .filter(|_| on && !hidden)?;
        let color = opaque(match indicator {
            Indicator::CapsLock => self.colors.caps_lock,
            Indicator::ScrollLock => self.colors.scroll_lock,
            Indicator::NumLock => self.colors.num_lock,
            Indicator::LayerLock => self.colors.layer_lock,
        });
        Some(Overlay::Solid { key, color })
    }
}
