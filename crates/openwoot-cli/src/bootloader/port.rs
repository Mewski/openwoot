use std::time::Duration;

pub const REQUEST_LEN: usize = 7;
pub const REPLY_LEN: usize = 256;

pub type Request = [u8; REQUEST_LEN];
pub type Reply = [u8; REPLY_LEN];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    #[error("the keyboard was disconnected")]
    Disconnected,
    #[error("could not communicate over USB: {0}")]
    Transfer(String),
}

pub trait Port {
    fn discard_replies(&mut self) -> impl Future<Output = Result<(), PortError>>;

    fn send_request(&mut self, request: &Request) -> impl Future<Output = Result<(), PortError>>;

    fn send_block(&mut self, block: &[u8]) -> impl Future<Output = Result<(), PortError>>;

    fn receive_reply(
        &mut self,
        reply: &mut Reply,
        timeout: Duration,
    ) -> impl Future<Output = Result<Option<usize>, PortError>>;

    fn pause(&mut self, duration: Duration) -> impl Future<Output = ()>;
}
