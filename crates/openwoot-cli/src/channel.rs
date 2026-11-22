use std::time::{Duration, Instant};

use openwoot_protocol::{GetVersion, MAX_REPORT_LEN, Version, decode_response, encode_request};

use crate::{bootloader::PortError, hid::Interface};

const SEQUENCE_NUMBER: u16 = 1;
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Port(#[from] PortError),
    #[error("could not encode the request")]
    Encode(#[source] openwoot_protocol::Error),
    #[error("the firmware answered with an error")]
    Response(#[source] openwoot_protocol::Error),
}

pub async fn firmware_version(interface: &mut Interface) -> Result<Option<Version>, Error> {
    let mut request = [0; MAX_REPORT_LEN];
    let report =
        encode_request::<GetVersion>(SEQUENCE_NUMBER, &(), &mut request).map_err(Error::Encode)?;
    interface.discard_input().await?;
    interface.write_output(report).await?;
    let deadline = Instant::now() + REPLY_TIMEOUT;
    let mut reply = vec![0; MAX_REPORT_LEN];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Some(len) = interface.read_input(&mut reply, remaining).await? else {
            return Ok(None);
        };
        let report = reply.get(..len).unwrap_or_default();
        if let Some(version) =
            decode_response::<GetVersion>(SEQUENCE_NUMBER, report).map_err(Error::Response)?
        {
            return Ok(Some(version));
        }
    }
}
