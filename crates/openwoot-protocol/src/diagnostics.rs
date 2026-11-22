use core::fmt;

use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Report {
    pub scan: Scan,
    pub keys: Keys,
    pub profile: Profile,
    pub lighting: Lighting,
    pub led_bus: Bus,
    pub storage: Storage,
    pub flash_bus: Bus,
    pub usb: Usb,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Scan {
    pub cycles: u32,
    pub recoveries: u32,
    pub overruns: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Keys {
    pub passes: u32,
    pub reports: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Profile {
    pub active: Option<u8>,
    pub requests: u32,
    pub switches: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Lighting {
    pub initialized: bool,
    pub active: bool,
    pub frames: u32,
    pub page_writes: u32,
    pub failed_page_writes: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Bus {
    pub completed: u32,
    pub errors: BusErrors,
    pub last_error: Option<BusError>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct BusErrors {
    pub timeout: u32,
    pub bus: u32,
    pub arbitration_loss: u32,
    pub no_acknowledge: u32,
    pub overrun: u32,
    pub dma: u32,
    pub other: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, thiserror::Error)]
pub enum BusError {
    #[error("timeout")]
    Timeout,
    #[error("bus error")]
    Bus,
    #[error("arbitration loss")]
    ArbitrationLoss,
    #[error("no acknowledge")]
    NoAcknowledge,
    #[error("overrun")]
    Overrun,
    #[error("DMA error")]
    Dma,
    #[error("other")]
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Storage {
    pub loads: u32,
    pub stores: u32,
    pub failures: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Usb {
    pub configured: bool,
    pub suspended: bool,
    pub speed: Option<Speed>,
    pub resets: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Speed {
    Low,
    Full,
    High,
}

impl BusErrors {
    pub const KINDS: [BusError; 7] = [
        BusError::Timeout,
        BusError::Bus,
        BusError::ArbitrationLoss,
        BusError::NoAcknowledge,
        BusError::Overrun,
        BusError::Dma,
        BusError::Other,
    ];

    #[must_use]
    pub const fn count(&self, kind: BusError) -> u32 {
        match kind {
            BusError::Timeout => self.timeout,
            BusError::Bus => self.bus,
            BusError::ArbitrationLoss => self.arbitration_loss,
            BusError::NoAcknowledge => self.no_acknowledge,
            BusError::Overrun => self.overrun,
            BusError::Dma => self.dma,
            BusError::Other => self.other,
        }
    }

    #[must_use]
    pub fn total(&self) -> u32 {
        Self::KINDS
            .iter()
            .fold(0, |total, &kind| total.saturating_add(self.count(kind)))
    }
}

impl fmt::Display for Speed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Low => "low speed (1.5 Mbit/s)",
            Self::Full => "full speed (12 Mbit/s)",
            Self::High => "high speed (480 Mbit/s)",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_errors_add_up_over_every_kind() {
        let errors = BusErrors {
            timeout: 2,
            no_acknowledge: 3,
            other: 1,
            ..BusErrors::default()
        };
        assert_eq!(errors.total(), 6);
        assert_eq!(errors.count(BusError::NoAcknowledge), 3);
        assert_eq!(errors.count(BusError::Dma), 0);
    }
}
