use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("the keyboard was disconnected")]
    Disconnected,
    #[error("access to the device was denied: {0}")]
    NoAccess(String),
    #[error("could not communicate over USB: {0}")]
    Failed(String),
}

pub trait Reports {
    fn write_output(&mut self, report: &[u8]) -> impl Future<Output = Result<(), Error>>;

    fn write_feature(&mut self, report: &[u8]) -> impl Future<Output = Result<(), Error>>;

    fn read_input(
        &mut self,
        report: &mut [u8],
        timeout: Duration,
    ) -> impl Future<Output = Result<Option<usize>, Error>>;

    fn discard_input(&mut self) -> impl Future<Output = Result<(), Error>>;
}
