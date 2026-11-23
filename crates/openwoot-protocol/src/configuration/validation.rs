use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use super::Capabilities;
use crate::{
    device::Layout,
    lightbar::{MAX_HOLD_MS, MAX_HOST_SIGNALS},
};

pub trait Validate {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, thiserror::Error)]
pub enum Invalid {
    #[error("expected {expected} keys, found {found}")]
    KeyCountDiffersFromMatrix { expected: u16, found: u16 },
    #[error("key {0} lies outside the key matrix")]
    KeyOutsideMatrix(u8),
    #[error("row {row} column {column} lies outside the key matrix")]
    PositionOutsideMatrix { row: u8, column: u8 },
    #[error("key {0} is paired with itself")]
    KeyPairedWithItself(u8),
    #[error("key {0} has more than one advanced key on a layer")]
    KeyBoundTwice(u8),
    #[error("keyboard usage `{0:#04x}` has no place in the keyboard report")]
    KeyboardUsageOutsideReport(u8),
    #[error("consumer usage `{0:#06x}` lies outside `0x0001`..`0x02a2`")]
    ConsumerUsageOutsideRange(u16),
    #[error("profile {0} lies outside the profiles of the keyboard")]
    ProfileOutsideRange(u8),
    #[error("the base layer cannot be a momentary layer")]
    MomentaryBaseLayer,
    #[error("the distance is zero")]
    ZeroDistance,
    #[error("the hold time is zero")]
    ZeroHoldTime,
    #[error("a hold time of {0} ms is longer than {limit} ms", limit = MAX_HOLD_MS)]
    HoldTimeTooLong(u32),
    #[error("the duration is zero")]
    ZeroDuration,
    #[error("the response curve does not rise from the dead zone to the saturation point")]
    CurveNotRising,
    #[error("{0} % is more than 100 %")]
    PercentageAboveHundred(u8),
    #[error("{value} lies outside {min}..={max}")]
    RateOutsideRange { value: u8, min: u8, max: u8 },
    #[error("light bar positions {first}..={last} lie outside the {leds} LEDs")]
    LightbarRangeOutsideLeds { first: u8, last: u8, leds: u8 },
    #[error("{count} colours do not fit the {leds} light bar LEDs")]
    LightbarColorsExceedLeds { count: u8, leds: u8 },
    #[error(
        "the light bar entries refer to more than {} host signals",
        MAX_HOST_SIGNALS
    )]
    LightbarHostSignalsExceedLimit,
    #[error("the key rectangle ends before it starts")]
    KeyRectInverted,
    #[error("the condition can never hold")]
    ConditionNeverHolds,
    #[error("the period is zero")]
    ZeroPeriod,
    #[error("the meter maximum is not a positive number")]
    MeterMaximumNotPositive,
    #[error("the profile indicator has no segments")]
    ZeroSegments,
    #[error("the keyboard has no light bar")]
    NoLightbar,
    #[error("the keyboard takes no host signals")]
    NoHostSignals,
    #[error("switch model {0} is unknown to the keyboard")]
    SwitchModelUnknown(u8),
    #[error("layout `{0:?}` does not exist on the keyboard")]
    LayoutMissing(Layout),
}

impl<T: Validate> Validate for Option<T> {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.as_ref()
            .map_or(Ok(()), |value| value.validate(capabilities))
    }
}
