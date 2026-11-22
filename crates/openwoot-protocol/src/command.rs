use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    GetVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    Version {
        protocol: u16,
        firmware: FirmwareVersion,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_FRAME_LEN;

    #[test]
    fn request_and_response_survive_a_round_trip() {
        let mut buffer = [0; MAX_FRAME_LEN];
        let request = Request::GetVersion;
        let encoded = postcard::to_slice(&request, &mut buffer).unwrap();
        assert_eq!(postcard::from_bytes::<Request>(encoded).unwrap(), request);

        let response = Response::Version {
            protocol: PROTOCOL_VERSION,
            firmware: FirmwareVersion {
                major: 0,
                minor: 1,
                patch: 2,
            },
        };
        let encoded = postcard::to_slice(&response, &mut buffer).unwrap();
        assert_eq!(postcard::from_bytes::<Response>(encoded).unwrap(), response);
    }
}
