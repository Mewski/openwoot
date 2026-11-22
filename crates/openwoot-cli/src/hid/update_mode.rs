use std::time::Duration;

use async_io::Timer;

use super::Interface;
use crate::{
    bootloader::{Port, REQUEST_LEN, Reply, Request},
    transfer::{self, Reports},
};

const NO_REPORT_ID: u8 = 0;

pub struct UpdateMode(Interface);

impl UpdateMode {
    #[must_use]
    pub const fn new(interface: Interface) -> Self {
        Self(interface)
    }
}

impl Port for UpdateMode {
    async fn discard_replies(&mut self) -> Result<(), transfer::Error> {
        self.0.discard_input().await
    }

    async fn send_request(&mut self, request: &Request) -> Result<(), transfer::Error> {
        let mut report = [NO_REPORT_ID; 1 + REQUEST_LEN];
        if let Some(body) = report.get_mut(1..) {
            body.copy_from_slice(request);
        }
        self.0.write_feature(&report).await
    }

    async fn send_block(&mut self, block: &[u8]) -> Result<(), transfer::Error> {
        let report: Vec<u8> = [NO_REPORT_ID].iter().chain(block).copied().collect();
        self.0.write_output(&report).await
    }

    async fn receive_reply(
        &mut self,
        reply: &mut Reply,
        timeout: Duration,
    ) -> Result<Option<usize>, transfer::Error> {
        self.0.read_input(reply, timeout).await
    }

    async fn pause(&mut self, duration: Duration) {
        Timer::after(duration).await;
    }
}
