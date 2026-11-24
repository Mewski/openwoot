use std::{
    process,
    sync::atomic::{AtomicU16, Ordering},
    time::{Duration, Instant},
};

use openwoot_protocol::{
    Endpoint, Version, WireError, decode_response, encode_request,
    endpoints::{EnterUpdateMode, GetVersion},
    report,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::transfer::{self, Reports};

const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

static REQUESTS: AtomicU16 = AtomicU16::new(0);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Transfer(#[from] transfer::Error),
    #[error("could not encode the request")]
    Encode(#[source] openwoot_protocol::Error),
    #[error("could not use the reply")]
    Reply(#[source] openwoot_protocol::Error),
    #[error("the keyboard did not answer")]
    NoReply,
    #[error("the keyboard answered with the error `{0:?}`")]
    Device(WireError),
}

pub async fn request<E>(
    reports: &mut impl Reports,
    request: &E::Request,
) -> Result<E::Response, Error>
where
    E: Endpoint,
    E::Request: Serialize,
    E::Response: DeserializeOwned,
{
    let sequence_number = next_sequence_number();
    let mut encoded = [0; report::MAX_LEN];
    let sent =
        encode_request::<E>(sequence_number, request, &mut encoded).map_err(Error::Encode)?;
    reports.discard_input().await?;
    reports.write_output(sent).await?;
    let deadline = Instant::now() + REPLY_TIMEOUT;
    let mut input = vec![0; report::MAX_LEN];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Some(len) = reports.read_input(&mut input, remaining).await? else {
            return Err(Error::NoReply);
        };
        let received = input.get(..len).unwrap_or_default();
        match decode_response::<E>(sequence_number, received) {
            Ok(Some(response)) => return Ok(response),
            Ok(None) => {}
            Err(openwoot_protocol::Error::Device(error)) => return Err(Error::Device(error)),
            Err(error) => return Err(Error::Reply(error)),
        }
    }
}

pub(crate) async fn firmware_version(reports: &mut impl Reports) -> Result<Version, Error> {
    request::<GetVersion>(reports, &()).await
}

pub(crate) async fn enter_update_mode(reports: &mut impl Reports) -> Result<(), Error> {
    request::<EnterUpdateMode>(reports, &()).await
}

fn next_sequence_number() -> u16 {
    let [low, high, ..] = process::id().to_le_bytes();
    u16::from_le_bytes([low, high]).wrapping_add(REQUESTS.fetch_add(1, Ordering::Relaxed))
}
