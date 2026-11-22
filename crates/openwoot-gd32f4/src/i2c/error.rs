use embedded_hal_async::i2c::{self as hal_i2c, ErrorKind, NoAcknowledgeSource};

pub(super) const STAT0_ERRORS: u32 = STAT0_BERR | STAT0_LOSTARB | STAT0_AERR | STAT0_OUERR;

const STAT0_BERR: u32 = 1 << 8;
const STAT0_LOSTARB: u32 = 1 << 9;
const STAT0_AERR: u32 = 1 << 10;
const STAT0_OUERR: u32 = 1 << 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("the LED bus did not finish a transfer within 10 ms")]
    Timeout,
    #[error("the LED bus saw a misplaced start or stop condition")]
    Bus,
    #[error("the LED bus lost arbitration")]
    ArbitrationLoss,
    #[error("the device did not acknowledge its {0:?}")]
    NoAcknowledge(NoAcknowledgeSource),
    #[error("the LED bus over- or underran")]
    Overrun,
    #[error("the LED bus DMA channel reported a transfer error")]
    Dma,
    #[error("the LED bus only writes")]
    Read,
    #[error("a transfer of {0} bytes exceeds 65535 bytes")]
    TooLong(usize),
    #[error("a transaction of more than 4 writes")]
    TooManyWrites,
}

impl Error {
    pub(super) const fn from_status(status: u32, source: NoAcknowledgeSource) -> Option<Self> {
        if status & STAT0_AERR != 0 {
            Some(Self::NoAcknowledge(source))
        } else if status & STAT0_LOSTARB != 0 {
            Some(Self::ArbitrationLoss)
        } else if status & STAT0_BERR != 0 {
            Some(Self::Bus)
        } else if status & STAT0_OUERR != 0 {
            Some(Self::Overrun)
        } else {
            None
        }
    }
}

impl hal_i2c::Error for Error {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Bus => ErrorKind::Bus,
            Self::ArbitrationLoss => ErrorKind::ArbitrationLoss,
            Self::NoAcknowledge(source) => ErrorKind::NoAcknowledge(*source),
            Self::Overrun => ErrorKind::Overrun,
            Self::Timeout | Self::Dma | Self::Read | Self::TooLong(_) | Self::TooManyWrites => {
                ErrorKind::Other
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i2c::registers::STAT0_BTC;

    #[test]
    fn status_errors_map_to_their_kinds() {
        assert_eq!(Error::from_status(0, NoAcknowledgeSource::Data), None);
        assert_eq!(
            Error::from_status(STAT0_AERR | STAT0_BTC, NoAcknowledgeSource::Address),
            Some(Error::NoAcknowledge(NoAcknowledgeSource::Address))
        );
        assert_eq!(hal_i2c::Error::kind(&Error::Timeout), ErrorKind::Other);
        assert_eq!(
            Error::from_status(STAT0_LOSTARB, NoAcknowledgeSource::Data),
            Some(Error::ArbitrationLoss)
        );
    }
}
