use crate::{Error, report::smallest_report};

pub const MAGIC: [u8; 2] = [0xD1, 0xDA];
pub const FEATURE_REPORT_ID: u8 = 1;
pub const REQUEST_LEN: usize = 7;
pub const REPLY_HEADER_LEN: usize = 6;
pub const COMMAND_COUNT: u8 = 89;
pub const FIRST_CURRENT_COMMAND: u8 = 38;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    Ping = 0,
    Version = 1,
    EnterUpdateMode = 2,
    Serial = 3,
    DeviceSettings = 19,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    Unknown = 0x66,
    Busy = 0x77,
    Success = 0x88,
    Overflow = 0x99,
    Unsupported = 0xAA,
    Error = 0xFF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub command: u8,
    pub argument: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reply<'a> {
    pub command: u8,
    pub status: u8,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrongMagic {
    pub command: u8,
}

impl Command {
    #[must_use]
    pub const fn from_number(number: u8) -> Option<Self> {
        match number {
            0 => Some(Self::Ping),
            1 => Some(Self::Version),
            2 => Some(Self::EnterUpdateMode),
            3 => Some(Self::Serial),
            19 => Some(Self::DeviceSettings),
            _ => None,
        }
    }
}

impl From<Command> for u8 {
    fn from(command: Command) -> Self {
        command as Self
    }
}

impl From<Status> for u8 {
    fn from(status: Status) -> Self {
        status as Self
    }
}

impl Request {
    #[must_use]
    pub const fn new(command: Command, argument: u32) -> Self {
        Self {
            command: command as u8,
            argument,
        }
    }

    pub fn parse(data: &[u8]) -> Result<Self, WrongMagic> {
        let mut request = [0; REQUEST_LEN];
        for (byte, received) in request.iter_mut().zip(data) {
            *byte = *received;
        }
        let [first, second, command, argument @ ..] = request;
        if [first, second] != MAGIC {
            return Err(WrongMagic { command });
        }
        Ok(Self {
            command,
            argument: u32::from_le_bytes(argument),
        })
    }

    #[must_use]
    pub fn to_report(self) -> [u8; 1 + REQUEST_LEN] {
        let [first, second] = MAGIC;
        let [a0, a1, a2, a3] = self.argument.to_le_bytes();
        [
            FEATURE_REPORT_ID,
            first,
            second,
            self.command,
            a0,
            a1,
            a2,
            a3,
        ]
    }
}

pub fn encode_reply<'a>(
    command: u8,
    status: Status,
    payload: &[u8],
    report: &'a mut [u8],
) -> Result<&'a [u8], Error> {
    let frame_len = REPLY_HEADER_LEN + payload.len();
    let (report_id, size) = smallest_report(frame_len).ok_or(Error::FrameTooLong)?;
    let length = u16::try_from(payload.len()).map_err(|_| Error::FrameTooLong)?;
    let report = report.get_mut(..=size).ok_or(Error::FrameTooLong)?;
    let (head, tail) = report
        .split_at_mut_checked(1 + REPLY_HEADER_LEN)
        .ok_or(Error::FrameTooLong)?;
    let [first, second] = MAGIC;
    let [low, high] = length.to_le_bytes();
    head.copy_from_slice(&[report_id, first, second, command, status.into(), low, high]);
    let (body, padding) = tail
        .split_at_mut_checked(payload.len())
        .ok_or(Error::FrameTooLong)?;
    body.copy_from_slice(payload);
    padding.fill(0);
    Ok(report)
}

#[must_use]
pub fn decode_reply(report: &[u8]) -> Option<Reply<'_>> {
    match report {
        [_, first, second, command, status, low, high, rest @ ..] if [*first, *second] == MAGIC => {
            let length = usize::from(u16::from_le_bytes([*low, *high]));
            Some(Reply {
                command: *command,
                status: *status,
                payload: rest.get(..length)?,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_reads_its_argument_little_endian() {
        assert_eq!(
            Request::parse(&[0xD1, 0xDA, 0x4C, 0x01, 0x01, 0x00, 0x00]),
            Ok(Request {
                command: 76,
                argument: 0x0101
            })
        );
        assert_eq!(
            Request::new(Command::EnterUpdateMode, 4).to_report(),
            [1, 0xD1, 0xDA, 2, 4, 0, 0, 0]
        );
    }

    #[test]
    fn a_short_request_reads_as_zero_padded_and_a_wrong_magic_keeps_the_command() {
        assert_eq!(
            Request::parse(&[0xD1, 0xDA, 3]),
            Ok(Request {
                command: 3,
                argument: 0
            })
        );
        assert_eq!(
            Request::parse(&[0xDA, 0xD1, 1, 0, 0, 0, 0]),
            Err(WrongMagic { command: 1 })
        );
    }

    #[test]
    fn a_reply_goes_into_the_smallest_input_report() {
        let mut report = [0xAA; 64];
        let reply = encode_reply(1, Status::Success, &[2, 14, 1], &mut report).unwrap();
        assert_eq!(reply.len(), 33);
        assert_eq!(reply[..10], [1, 0xD1, 0xDA, 1, 0x88, 3, 0, 2, 14, 1]);
        assert!(reply[10..].iter().all(|&byte| byte == 0));
        assert_eq!(
            decode_reply(reply),
            Some(Reply {
                command: 1,
                status: 0x88,
                payload: &[2, 14, 1]
            })
        );

        let payload = [7; 30];
        let mut report = [0; 64];
        let reply = encode_reply(3, Status::Success, &payload, &mut report).unwrap();
        assert_eq!((reply[0], reply.len()), (2, 63));
    }
}
