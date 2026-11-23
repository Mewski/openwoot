use std::{
    process,
    sync::atomic::{AtomicU16, Ordering},
    time::{Duration, Instant},
};

use openwoot_protocol::{
    Endpoint, Version, decode_response,
    diagnostics::Report,
    encode_request,
    endpoints::{EnterUpdateMode, GetDiagnostics, GetVersion},
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
}

pub(crate) async fn firmware_version(reports: &mut impl Reports) -> Result<Option<Version>, Error> {
    request::<GetVersion>(reports, &()).await
}

pub(crate) async fn enter_update_mode(reports: &mut impl Reports) -> Result<(), Error> {
    request::<EnterUpdateMode>(reports, &()).await?;
    Ok(())
}

pub async fn diagnostics(reports: &mut impl Reports) -> Result<Option<Report>, Error> {
    request::<GetDiagnostics>(reports, &()).await
}

async fn request<E>(
    reports: &mut impl Reports,
    request: &E::Request,
) -> Result<Option<E::Response>, Error>
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
            return Ok(None);
        };
        let received = input.get(..len).unwrap_or_default();
        if let Some(response) =
            decode_response::<E>(sequence_number, received).map_err(Error::Reply)?
        {
            return Ok(Some(response));
        }
    }
}

fn next_sequence_number() -> u16 {
    let [low, high, ..] = process::id().to_le_bytes();
    u16::from_le_bytes([low, high]).wrapping_add(REQUESTS.fetch_add(1, Ordering::Relaxed))
}
