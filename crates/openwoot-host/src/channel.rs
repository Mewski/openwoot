use std::{
    process,
    sync::atomic::{AtomicU16, Ordering},
    time::{Duration, Instant},
};

use openwoot_protocol::{
    Endpoint, Version, WireError, decode_response, encode_request,
    endpoints::{EnterUpdateMode, FactoryReset, GetVersion, Save},
    report,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::transfer::{self, Reports};

const REPLY_TIMEOUT: Duration = Duration::from_secs(2);
const SAVE_TIMEOUT: Duration = Duration::from_secs(10);
const FACTORY_RESET_TIMEOUT: Duration = Duration::from_secs(60);

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
    let deadline = Instant::now() + reply_timeout::<E>();
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

fn reply_timeout<E: Endpoint>() -> Duration {
    if E::PATH == FactoryReset::PATH {
        FACTORY_RESET_TIMEOUT
    } else if E::PATH == Save::PATH {
        SAVE_TIMEOUT
    } else {
        REPLY_TIMEOUT
    }
}

fn next_sequence_number() -> u16 {
    let [low, high, ..] = process::id().to_le_bytes();
    u16::from_le_bytes([low, high]).wrapping_add(REQUESTS.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::endpoints::{GetProfiles, Revert};

    use super::*;

    #[test]
    fn a_factory_reset_waits_for_the_whole_settings_area_to_erase() {
        let sectors = 128;
        let slowest_sector_erase = Duration::from_millis(400);
        assert!(reply_timeout::<FactoryReset>() >= slowest_sector_erase * sectors);
    }

    #[test]
    fn saving_waits_longer_than_other_requests() {
        assert!(reply_timeout::<Save>() > REPLY_TIMEOUT);
        assert_eq!(reply_timeout::<GetProfiles>(), REPLY_TIMEOUT);
        assert_eq!(reply_timeout::<Revert>(), REPLY_TIMEOUT);
    }
}
