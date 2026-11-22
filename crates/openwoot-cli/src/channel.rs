use std::time::{Duration, Instant};

use openwoot_protocol::{
    Endpoint, Version, decode_response, encode_request,
    endpoints::{EnterUpdateMode, GetVersion},
    report,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::transfer::{self, Reports};

const SEQUENCE_NUMBER: u16 = 1;
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Transfer(#[from] transfer::Error),
    #[error("could not encode the request")]
    Encode(#[source] openwoot_protocol::Error),
    #[error("the firmware answered with an error")]
    Response(#[source] openwoot_protocol::Error),
}

pub(crate) async fn firmware_version(reports: &mut impl Reports) -> Result<Option<Version>, Error> {
    request::<GetVersion>(reports, &()).await
}

pub(crate) async fn enter_update_mode(reports: &mut impl Reports) -> Result<(), Error> {
    request::<EnterUpdateMode>(reports, &()).await?;
    Ok(())
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
    let mut encoded = [0; report::MAX_LEN];
    let report =
        encode_request::<E>(SEQUENCE_NUMBER, request, &mut encoded).map_err(Error::Encode)?;
    reports.discard_input().await?;
    reports.write_output(report).await?;
    let deadline = Instant::now() + REPLY_TIMEOUT;
    let mut reply = vec![0; report::MAX_LEN];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Some(len) = reports.read_input(&mut reply, remaining).await? else {
            return Ok(None);
        };
        let report = reply.get(..len).unwrap_or_default();
        if let Some(response) =
            decode_response::<E>(SEQUENCE_NUMBER, report).map_err(Error::Response)?
        {
            return Ok(Some(response));
        }
    }
}
