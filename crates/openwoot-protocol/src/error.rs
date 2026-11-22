use postcard_rpc::standard_icd::WireError;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("the frame does not fit in a report")]
    FrameTooLong,
    #[error("the report is not a valid frame")]
    MalformedReport,
    #[error("could not encode the message")]
    Encode,
    #[error("could not decode the message")]
    Decode,
    #[error("the keyboard answered with the error `{0:?}`")]
    Device(WireError),
}
