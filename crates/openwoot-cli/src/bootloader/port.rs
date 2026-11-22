use std::time::Duration;

use crate::transfer;

pub const REQUEST_LEN: usize = 7;
pub const REPLY_LEN: usize = 256;

pub type Request = [u8; REQUEST_LEN];
pub type Reply = [u8; REPLY_LEN];

pub trait Port {
    fn discard_replies(&mut self) -> impl Future<Output = Result<(), transfer::Error>>;

    fn send_request(
        &mut self,
        request: &Request,
    ) -> impl Future<Output = Result<(), transfer::Error>>;

    fn send_block(&mut self, block: &[u8]) -> impl Future<Output = Result<(), transfer::Error>>;

    fn receive_reply(
        &mut self,
        reply: &mut Reply,
        timeout: Duration,
    ) -> impl Future<Output = Result<Option<usize>, transfer::Error>>;

    fn pause(&mut self, duration: Duration) -> impl Future<Output = ()>;
}
