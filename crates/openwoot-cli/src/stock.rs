use std::time::{Duration, Instant};

use openwoot_protocol::{
    report,
    stock::{Command, Request, Status, decode_reply},
};

use crate::{
    report::to_hex,
    transfer::{self, Reports},
};

const REPLY_TIMEOUT: Duration = Duration::from_secs(2);
const VERSION_LEN: usize = 3;
const SHOWN_REPLY_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Transfer(#[from] transfer::Error),
    #[error("unexpected reply to feature command {command}: `{reply}`")]
    UnexpectedReply { command: u8, reply: String },
}

pub(crate) async fn firmware_version(reports: &mut impl Reports) -> Result<Option<String>, Error> {
    let Some(payload) = command(reports, Command::Version).await? else {
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

pub(crate) async fn enter_update_mode(reports: &mut impl Reports) {
    send(reports, Command::EnterUpdateMode).await.ok();
}

async fn command(reports: &mut impl Reports, command: Command) -> Result<Option<Vec<u8>>, Error> {
    send(reports, command).await?;
    let number = u8::from(command);
    let deadline = Instant::now() + REPLY_TIMEOUT;
    let mut buffer = vec![0; report::MAX_LEN];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Some(len) = reports.read_input(&mut buffer, remaining).await? else {
            return Ok(None);
        };
        let received = buffer.get(..len).unwrap_or_default();
        match decode_reply(received) {
            Some(reply) if reply.command == number && reply.status == u8::from(Status::Success) => {
                return Ok(Some(reply.payload.to_vec()));
            }
            Some(reply) if reply.command == number => {
                return Err(Error::UnexpectedReply {
                    command: number,
                    reply: to_hex(received.get(..SHOWN_REPLY_LEN).unwrap_or(received)),
                });
            }
            _ => {}
        }
    }
}

async fn send(reports: &mut impl Reports, command: Command) -> Result<(), transfer::Error> {
    reports.discard_input().await?;
    reports
        .write_feature(&Request::new(command, 0).to_report())
        .await
}
