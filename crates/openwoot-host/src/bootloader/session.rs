use std::{fmt, time::Duration};

use openwoot_image::Hex;

use super::{
    BLOCK_LEN, Command, FIRST_BLOCK_ADDRESS, MAX_BLOCKS, Port, REPLY_LEN, REQUEST_LEN, Upload,
};
use crate::{report, transfer};

const MAGIC: [u8; 2] = [0xDF, 0xDB];
const STATUS_EXECUTED: u8 = 0x88;
const ECHO_ANY: u8 = 0xFF;
const REPLY_TIMEOUT: Duration = Duration::from_secs(4);
const PING_TIMEOUT: Duration = Duration::from_secs(1);
const PING_ATTEMPTS: usize = 5;
const PING_RETRY: Duration = Duration::from_millis(500);
const UNLOCK_KEY: u32 = 0xFFAA_FFBB;
const MARK_COMPLETE: u32 = 1;
const CRC_LEN: usize = 3;
const BLOCK_INDEX_LEN: usize = 4;
const SHOWN_REPLY_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Transfer(#[from] transfer::Error),
    #[error("no reply to {command} within {} ms", timeout.as_millis())]
    NoReply { command: Command, timeout: Duration },
    #[error("unexpected reply to {command}: `{reply}`")]
    UnexpectedReply { command: Command, reply: String },
    #[error("the reply to {command} holds {len} bytes, expected at least {expected}")]
    ShortReply {
        command: Command,
        len: usize,
        expected: usize,
    },
    #[error("{command} was refused with status {status:#04x}")]
    Refused { command: Command, status: u8 },
    #[error(
        "the image names blocks of {0} bytes; the bootloader takes blocks of {expected} bytes",
        expected = BLOCK_LEN
    )]
    BlockSizeUnsupported(u32),
    #[error("the image has {0} blocks; the bootloader takes 1 to {max}", max = MAX_BLOCKS)]
    BlockCountOutOfRange(usize),
    #[error(
        "the image area starts at {}; the bootloader writes the first block to {}",
        Hex(*.0),
        Hex(FIRST_BLOCK_ADDRESS)
    )]
    MisplacedImageArea(u32),
    #[error(
        "the upload of {len} bytes does not fit the image area {}-{} of {capacity} bytes",
        Hex(*start),
        Hex(*end)
    )]
    BeyondImageArea {
        len: usize,
        start: u32,
        end: u32,
        capacity: u32,
    },
    #[error("the bootloader reports block {reported} as the last one written, expected {expected}")]
    LastBlockMismatch { reported: u32, expected: u32 },
    #[error(
        "the bootloader reports CRC-24 {}, the image has {}",
        Hex(*reported),
        Hex(*expected)
    )]
    CrcMismatch { reported: u32, expected: u32 },
    #[error("could not send the restart command")]
    Restart(#[source] transfer::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags(u8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub flags: Option<Flags>,
    pub version: Option<String>,
    pub serial: Option<String>,
    pub session_crc: Option<u32>,
}

pub struct Session<P> {
    port: P,
}

impl Flags {
    const NAMES: [(u8, &str); 3] = [
        (0x01, "recovery key combination held"),
        (0x02, "update mode requested by the firmware"),
        (0x04, "no valid firmware installed"),
    ];
}

impl fmt::Display for Flags {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = Self::NAMES
            .iter()
            .filter(|(mask, _)| self.0 & mask != 0)
            .map(|(_, name)| *name)
            .collect();
        let names = if names.is_empty() {
            "none".to_owned()
        } else {
            names.join(", ")
        };
        write!(formatter, "{:#04x} ({names})", self.0)
    }
}

impl<P: Port> Session<P> {
    pub const fn new(port: P) -> Self {
        Self { port }
    }

    pub async fn state(&mut self) -> Result<State, Error> {
        let ping = self.ping().await?;
        let flags = ping.get(1).copied().map(Flags);
        let version = unless_refused(self.request(Command::Version, 0).await)?.map(|version| {
            version
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(".")
        });
        let serial = unless_refused(self.request(Command::Serial, 0).await)?;
        let second = unless_refused(self.request(Command::Serial, 1).await)?;
        let serial = match (serial.as_deref().map(text), second.as_deref().map(text)) {
            (Some(serial), Some(second)) if second != serial => {
                Some(format!("{serial} (with parameter 1: {second})"))
            }
            (serial, second) => serial.or(second),
        };
        let session_crc = unless_refused(self.crc().await)?;
        Ok(State {
            flags,
            version,
            serial,
            session_crc,
        })
    }

    pub async fn install(
        &mut self,
        upload: &Upload,
        mut on_block: impl FnMut(usize),
    ) -> Result<(), Error> {
        if let Some(odd) = upload
            .blocks()
            .map(<[u8]>::len)
            .find(|&len| len != BLOCK_LEN)
        {
            return Err(Error::BlockSizeUnsupported(
                u32::try_from(odd).unwrap_or(u32::MAX),
            ));
        }
        let count = upload.block_count();
        let blocks = u16::try_from(count)
            .ok()
            .filter(|&blocks| (1..=MAX_BLOCKS).contains(&usize::from(blocks)))
            .ok_or(Error::BlockCountOutOfRange(count))?;
        self.request(Command::Unlock, UNLOCK_KEY).await?;
        self.request(Command::StartUpload, u32::from(blocks))
            .await?;
        for (index, block) in (0..blocks).zip(upload.blocks()) {
            self.port.send_block(block).await?;
            self.request(Command::WriteBlock, u32::from(index)).await?;
            on_block(usize::from(index) + 1);
        }
        self.verify(blocks, upload.crc).await?;
        self.send(Command::Reboot, MARK_COMPLETE)
            .await
            .map_err(Error::Restart)
    }

    async fn ping(&mut self) -> Result<Vec<u8>, Error> {
        let mut attempt = 1;
        loop {
            match self.request_within(Command::Ping, 0, PING_TIMEOUT).await {
                Err(error)
                    if attempt < PING_ATTEMPTS
                        && error != Error::Transfer(transfer::Error::Disconnected) =>
                {
                    attempt += 1;
                    self.port.pause(PING_RETRY).await;
                }
                result => return result,
            }
        }
    }

    async fn verify(&mut self, blocks: u16, expected_crc: u32) -> Result<(), Error> {
        let expected = u32::from(blocks) - 1;
        let last = self.request(Command::LastBlock, 0).await?;
        let index = last
            .first_chunk::<BLOCK_INDEX_LEN>()
            .ok_or(Error::ShortReply {
                command: Command::LastBlock,
                len: last.len(),
                expected: BLOCK_INDEX_LEN,
            })?;
        let reported = little_endian(index);
        if reported != expected {
            return Err(Error::LastBlockMismatch { reported, expected });
        }
        let reported = self.crc().await?;
        if reported != expected_crc {
            return Err(Error::CrcMismatch {
                reported,
                expected: expected_crc,
            });
        }
        Ok(())
    }

    async fn crc(&mut self) -> Result<u32, Error> {
        let body = self.request(Command::Crc, 0).await?;
        let crc = body.first_chunk::<CRC_LEN>().ok_or(Error::ShortReply {
            command: Command::Crc,
            len: body.len(),
            expected: CRC_LEN,
        })?;
        Ok(little_endian(crc))
    }

    async fn request(&mut self, command: Command, parameter: u32) -> Result<Vec<u8>, Error> {
        self.request_within(command, parameter, REPLY_TIMEOUT).await
    }

    async fn request_within(
        &mut self,
        command: Command,
        parameter: u32,
        timeout: Duration,
    ) -> Result<Vec<u8>, Error> {
        self.send(command, parameter).await?;
        let mut reply = [0; REPLY_LEN];
        let len = self
            .port
            .receive_reply(&mut reply, timeout)
            .await?
            .filter(|&len| len > 0)
            .ok_or(Error::NoReply { command, timeout })?;
        let reply = reply.get(..len).unwrap_or(&reply);
        match reply {
            [first, second, echo, status, length, body @ ..]
                if [*first, *second] == MAGIC
                    && (*echo == command.number() || *echo == ECHO_ANY) =>
            {
                if *status == STATUS_EXECUTED {
                    body.get(..usize::from(*length))
                        .map(<[u8]>::to_vec)
                        .ok_or_else(|| unexpected(command, reply))
                } else {
                    Err(Error::Refused {
                        command,
                        status: *status,
                    })
                }
            }
            _ => Err(unexpected(command, reply)),
        }
    }

    async fn send(&mut self, command: Command, parameter: u32) -> Result<(), transfer::Error> {
        self.port.discard_replies().await?;
        let mut request = [0; REQUEST_LEN];
        let (head, tail) = request.split_at_mut(MAGIC.len() + 1);
        head.copy_from_slice(&[MAGIC[0], MAGIC[1], command.number()]);
        tail.copy_from_slice(&parameter.to_le_bytes());
        self.port.send_request(&request).await
    }
}

fn unless_refused<T>(reply: Result<T, Error>) -> Result<Option<T>, Error> {
    match reply {
        Ok(body) => Ok(Some(body)),
        Err(Error::Refused { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

fn unexpected(command: Command, reply: &[u8]) -> Error {
    Error::UnexpectedReply {
        command,
        reply: report::to_hex(reply.get(..SHOWN_REPLY_LEN).unwrap_or(reply)),
    }
}

fn little_endian(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .rev()
        .fold(0, |value, &byte| value << 8 | u32::from(byte))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
