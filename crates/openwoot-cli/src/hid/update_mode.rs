use std::time::Duration;

use async_io::Timer;

use crate::{
    bootloader::{Port, REQUEST_LEN, Reply, Request},
    transfer::{self, Reports},
};

const NO_REPORT_ID: u8 = 0;

pub struct UpdateMode<R>(R);

impl<R> UpdateMode<R> {
    #[must_use]
    pub const fn new(reports: R) -> Self {
        Self(reports)
    }
}

impl<R: Reports> Port for UpdateMode<R> {
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

#[cfg(test)]
mod tests {
    use std::future::ready;

    use futures_lite::future::block_on;

    use super::*;

    #[derive(Debug, Default)]
    struct Recorded {
        outputs: Vec<Vec<u8>>,
        features: Vec<Vec<u8>>,
    }

    impl Reports for Recorded {
        fn write_output(
            &mut self,
            report: &[u8],
        ) -> impl Future<Output = Result<(), transfer::Error>> {
            self.outputs.push(report.to_vec());
            ready(Ok(()))
        }

        fn write_feature(
            &mut self,
            report: &[u8],
        ) -> impl Future<Output = Result<(), transfer::Error>> {
            self.features.push(report.to_vec());
            ready(Ok(()))
        }

        fn read_input(
            &mut self,
            _report: &mut [u8],
            _timeout: Duration,
        ) -> impl Future<Output = Result<Option<usize>, transfer::Error>> {
            ready(Ok(None))
        }

        fn discard_input(&mut self) -> impl Future<Output = Result<(), transfer::Error>> {
            ready(Ok(()))
        }
    }

    #[test]
    fn requests_and_blocks_travel_behind_a_zero_report_id_byte() {
        let request = [0xDF, 0xDB, 6, 0xBB, 0xFF, 0xAA, 0xFF];
        let block: Vec<u8> = (0..=255).collect();
        let mut port = UpdateMode::new(Recorded::default());
        block_on(port.send_request(&request)).unwrap();
        block_on(port.send_block(&block)).unwrap();
        let Recorded { outputs, features } = port.0;
        assert_eq!(features, [vec![0, 0xDF, 0xDB, 6, 0xBB, 0xFF, 0xAA, 0xFF]]);
        let mut report = vec![0];
        report.extend(&block);
        assert_eq!(outputs, [report]);
    }
}
