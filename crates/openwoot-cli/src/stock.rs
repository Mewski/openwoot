use std::time::Duration;

use crate::{bootloader::PortError, hid::Interface, report};

const REPORT_ID: u8 = 1;
const MAGIC: [u8; 2] = [0xD1, 0xDA];
const VERSION: u8 = 1;
const ENTER_UPDATE_MODE: u8 = 2;
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);
const READ_LEN: usize = 2048;
const VERSION_LEN: usize = 3;
const SHOWN_REPLY_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Port(#[from] PortError),
    #[error("unexpected reply to feature command {command}: `{reply}`")]
    UnexpectedReply { command: u8, reply: String },
}

pub async fn firmware_version(interface: &mut Interface) -> Result<Option<String>, Error> {
    let Some(payload) = command(interface, VERSION).await? else {
        return Ok(None);
    };
    let version = payload
        .iter()
        .take(VERSION_LEN)
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(".");
    Ok(Some(version))
}

pub async fn enter_update_mode(interface: &mut Interface) {
    send(interface, ENTER_UPDATE_MODE).await.ok();
}

async fn command(interface: &mut Interface, command: u8) -> Result<Option<Vec<u8>>, Error> {
    send(interface, command).await?;
    let mut buffer = vec![0; READ_LEN];
    let Some(len) = interface.read_input(&mut buffer, REPLY_TIMEOUT).await? else {
        return Ok(None);
    };
    let reply = buffer.get(..len).unwrap_or_default();
    match reply {
        [] => Ok(None),
        [_, first, second, echo, _status, low, high, payload @ ..]
            if [*first, *second] == MAGIC && *echo == command =>
        {
            let length = usize::from(u16::from_le_bytes([*low, *high]));
            Ok(Some(payload.iter().take(length).copied().collect()))
        }
        _ => Err(Error::UnexpectedReply {
            command,
            reply: report::to_hex(reply.get(..SHOWN_REPLY_LEN).unwrap_or(reply)),
        }),
    }
}

async fn send(interface: &mut Interface, command: u8) -> Result<(), PortError> {
    interface.discard_input().await?;
    let argument = 0_u32.to_le_bytes();
    let mut report = vec![REPORT_ID, MAGIC[0], MAGIC[1], command];
    report.extend(argument);
    interface.write_feature(&report).await
}
