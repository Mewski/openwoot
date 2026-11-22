use super::Key;
use crate::lighting::{Indicator, KeyEvent, LockState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    Key {
        key: Key,
        event: KeyEvent,
    },
    ShowLayer(u8),
    Locks(LockState),
    Brightness {
        increase: bool,
        held: bool,
    },
    Disabled {
        key: Key,
        disabled: bool,
    },
    Toggle {
        key: Key,
        latched: bool,
        layer_key: bool,
    },
    ProfileBlink(Key),
    StopProfileBlink,
    IndicatorKey {
        indicator: Indicator,
        key: Option<Key>,
    },
}
