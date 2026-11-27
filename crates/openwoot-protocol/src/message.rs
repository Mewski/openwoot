use postcard_rpc::{
    Endpoint, Key,
    header::{VarHeader, VarKey, VarSeq},
    standard_icd::{ERROR_KEY, WireError},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    Error,
    report::{self, Contents, FRAME_OFFSET, MAX_FRAME_LEN},
    wire,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Message<'a> {
    pub header: VarHeader,
    pub body: &'a [u8],
}

impl Message<'_> {
    #[must_use]
    pub fn reply_header(&self, key: Key) -> VarHeader {
        let mut reply_key = VarKey::Key8(key);
        reply_key.shrink_to(self.header.key.kind());
        VarHeader {
            key: reply_key,
            seq_no: self.header.seq_no,
        }
    }
}

pub fn encode_message<'a, T>(
    header: &VarHeader,
    message: &T,
    report: &'a mut [u8],
) -> Result<&'a [u8], Error>
where
    T: Serialize + ?Sized,
{
    frame(header, report, |buffer| {
        postcard::to_slice(message, buffer)
            .map(|encoded| encoded.len())
            .map_err(|error| match error {
                postcard::Error::SerializeBufferFull => Error::FrameTooLong,
                _ => Error::Encode,
            })
    })
}

pub fn encode_frame<'a>(
    header: &VarHeader,
    body: &dyn wire::Encode,
    report: &'a mut [u8],
) -> Result<&'a [u8], Error> {
    frame(header, report, |buffer| {
        wire::encode(body, buffer).ok_or(Error::FrameTooLong)
    })
}

pub fn decode_message(report: &[u8]) -> Result<Option<Message<'_>>, Error> {
    match report::parse(report)? {
        Contents::Frame(frame) => {
            let (header, body) = VarHeader::take_from_slice(frame).ok_or(Error::MalformedReport)?;
            Ok(Some(Message { header, body }))
        }
        Contents::Foreign { .. } => Ok(None),
    }
}

pub fn encode_request<'a, E>(
    seq_no: u16,
    request: &E::Request,
    report: &'a mut [u8],
) -> Result<&'a [u8], Error>
where
    E: Endpoint,
    E::Request: Serialize,
{
    let header = VarHeader {
        key: VarKey::Key8(E::REQ_KEY),
        seq_no: VarSeq::Seq2(seq_no),
    };
    encode_message(&header, request, report)
}

pub fn decode_response<E>(seq_no: u16, report: &[u8]) -> Result<Option<E::Response>, Error>
where
    E: Endpoint,
    E::Response: DeserializeOwned,
{
    let Some(message) = decode_message(report)? else {
        return Ok(None);
    };
    let (VarSeq::Seq2(received), VarKey::Key8(key)) = (message.header.seq_no, message.header.key)
    else {
        return Ok(None);
    };
    if received != seq_no {
        return Ok(None);
    }
    if key == E::RESP_KEY {
        decode_body(message.body).map(Some)
    } else if key == ERROR_KEY {
        Err(Error::Device(decode_body::<WireError>(message.body)?))
    } else {
        Ok(None)
    }
}

fn decode_body<T: DeserializeOwned>(body: &[u8]) -> Result<T, Error> {
    match postcard::take_from_bytes(body) {
        Ok((value, [])) => Ok(value),
        _ => Err(Error::Decode),
    }
}

fn frame<'a>(
    header: &VarHeader,
    report: &'a mut [u8],
    body: impl FnOnce(&mut [u8]) -> Result<usize, Error>,
) -> Result<&'a [u8], Error> {
    let frame = report.get_mut(FRAME_OFFSET..).ok_or(Error::FrameTooLong)?;
    let frame_capacity = frame.len().min(MAX_FRAME_LEN);
    let frame = frame.get_mut(..frame_capacity).ok_or(Error::FrameTooLong)?;
    let (header_bytes, buffer) = header.write_to_slice(frame).ok_or(Error::FrameTooLong)?;
    let header_len = header_bytes.len();
    let body_len = body(buffer)?;
    report::seal(report, header_len + body_len)
}
