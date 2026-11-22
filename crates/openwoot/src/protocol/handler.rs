use openwoot_protocol::{
    ERROR_KEY, Endpoint, GetVersion, Message, VarKey, Version, WireError, decode_message,
    encode_message,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handler {
    version: Version,
}

impl Handler {
    #[must_use]
    pub const fn new(version: Version) -> Self {
        Self { version }
    }

    pub fn respond<'a>(&mut self, request: &[u8], response: &'a mut [u8]) -> Option<&'a [u8]> {
        let message = decode_message(request).ok()??;
        let encoded = if message.header.key == VarKey::Key8(GetVersion::REQ_KEY) {
            reply::<GetVersion>(&message, &self.version, response)
        } else {
            encode_message(
                &message.reply_header(ERROR_KEY),
                &WireError::UnknownKey,
                response,
            )
        };
        encoded.ok()
    }
}

fn reply<'a, E: Endpoint>(
    message: &Message<'_>,
    answer: &E::Response,
    response: &'a mut [u8],
) -> Result<&'a [u8], openwoot_protocol::Error>
where
    E::Response: serde::Serialize,
{
    encode_message(&message.reply_header(E::RESP_KEY), answer, response)
}
