mod containers;
mod reader;
mod scalars;
mod standard;
mod utf8;
mod writer;

pub use openwoot_derive::{Decode, Encode};
pub use reader::Reader;
pub use writer::Writer;

pub trait Encode {
    fn encode(&self, writer: &mut Writer<'_>);
}

pub trait Decode: Sized {
    fn decode(reader: &mut Reader<'_>) -> Self;
}

#[must_use]
pub fn encode(value: &dyn Encode, buffer: &mut [u8]) -> Option<usize> {
    let mut writer = Writer::new(buffer);
    value.encode(&mut writer);
    writer.finish()
}

#[must_use]
pub fn decode<T: Decode>(bytes: &[u8]) -> Option<T> {
    let mut reader = Reader::new(bytes);
    let value = T::decode(&mut reader);
    reader
        .finish()
        .is_some_and(<[u8]>::is_empty)
        .then_some(value)
}

#[must_use]
pub fn decode_prefix<T: Decode>(bytes: &[u8]) -> Option<T> {
    let mut reader = Reader::new(bytes);
    let value = T::decode(&mut reader);
    reader.finish().is_some().then_some(value)
}

#[must_use]
pub fn take<T: Decode>(bytes: &[u8]) -> Option<(T, &[u8])> {
    let mut reader = Reader::new(bytes);
    let value = T::decode(&mut reader);
    reader.finish().map(|rest| (value, rest))
}
