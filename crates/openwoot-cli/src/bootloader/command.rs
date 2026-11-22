use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Ping,
    Version,
    StartUpload,
    WriteBlock,
    Crc,
    Reboot,
    Unlock,
    LastBlock,
    Serial,
}

impl Command {
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Ping => 0,
            Self::Version => 1,
            Self::StartUpload => 2,
            Self::WriteBlock => 3,
            Self::Crc => 4,
            Self::Reboot => 5,
            Self::Unlock => 6,
            Self::LastBlock => 8,
            Self::Serial => 9,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ping => "ping",
            Self::Version => "version",
            Self::StartUpload => "start upload",
            Self::WriteBlock => "write block",
            Self::Crc => "CRC",
            Self::Reboot => "reboot",
            Self::Unlock => "unlock",
            Self::LastBlock => "last block",
            Self::Serial => "serial",
        }
    }
}

impl fmt::Display for Command {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "command {} ({})", self.number(), self.name())
    }
}
