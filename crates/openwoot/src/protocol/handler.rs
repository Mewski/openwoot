use openwoot_protocol::{
    ERROR_KEY, Endpoint, EnterUpdateMode, GetVersion, Message, VarKey, Version, WireError,
    decode_message, encode_message,
};

use super::{StockIdentity, requests::Queued, stock::Responder};

const UPDATE_MODE_OPTION: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handler {
    version: Version,
    stock: Responder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer<'a> {
    pub reply: Option<&'a [u8]>,
    pub update_mode: Option<u8>,
}

impl Handler {
    #[must_use]
    pub fn new(version: Version, identity: StockIdentity) -> Self {
        Self {
            version,
            stock: Responder::new(identity),
        }
    }

    pub fn respond<'a>(&mut self, request: &[u8], response: &'a mut [u8]) -> Answer<'a> {
        let Ok(Some(message)) = decode_message(request) else {
            return Answer::NONE;
        };
        let key = message.header.key;
        let (encoded, update_mode) = if key == VarKey::Key8(GetVersion::REQ_KEY) {
            (reply::<GetVersion>(&message, &self.version, response), None)
        } else if key == VarKey::Key8(EnterUpdateMode::REQ_KEY) {
            (
                reply::<EnterUpdateMode>(&message, &(), response),
                Some(UPDATE_MODE_OPTION),
            )
        } else {
            (
                encode_message(
                    &message.reply_header(ERROR_KEY),
                    &WireError::UnknownKey,
                    response,
                ),
                None,
            )
        };
        Answer {
            reply: encoded.ok(),
            update_mode,
        }
    }

    pub(crate) fn answer_stock<'a>(&self, queued: Queued, response: &'a mut [u8]) -> Answer<'a> {
        self.stock.answer(queued, response)
    }
}

impl Answer<'_> {
    pub const NONE: Self = Self {
        reply: None,
        update_mode: None,
    };
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
