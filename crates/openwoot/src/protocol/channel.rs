use core::pin::pin;

use embassy_futures::select::{Either, select};
use embassy_usb::driver::{EndpointError, EndpointIn, EndpointOut};
use openwoot_protocol::report;

use super::{Answer, Configuration, Handler, Handover, Restart, UpdateMode};
use crate::usb::FeatureRequests;

pub struct Channel<Output, Input> {
    output: Output,
    input: Input,
    request: [u8; report::MAX_LEN],
    response: [u8; report::MAX_LEN],
}

impl<Output: EndpointOut, Input: EndpointIn> Channel<Output, Input> {
    pub const fn new(output: Output, input: Input) -> Self {
        Self {
            output,
            input,
            request: [0; report::MAX_LEN],
            response: [0; report::MAX_LEN],
        }
    }

    pub async fn run<C: Configuration>(
        &mut self,
        handler: &mut Handler<C>,
        requests: &FeatureRequests,
        bootloader: &mut (impl UpdateMode + Restart),
    ) -> ! {
        loop {
            self.output.wait_enabled().await;
            while self.serve(handler, requests, bootloader).await != Err(EndpointError::Disabled) {}
        }
    }

    pub async fn serve<C: Configuration>(
        &mut self,
        handler: &mut Handler<C>,
        requests: &FeatureRequests,
        bootloader: &mut (impl UpdateMode + Restart),
    ) -> Result<(), EndpointError> {
        let Self {
            output,
            input,
            request,
            response,
        } = self;
        let length = {
            let mut read = pin!(output.read_transfer(request));
            loop {
                match select(read.as_mut(), requests.receive()).await {
                    Either::First(length) => break length?,
                    Either::Second(feature_request) => {
                        let answer = handler.answer_stock(feature_request, response).await;
                        deliver(input, answer, bootloader).await?;
                    }
                }
            }
        };
        let request = request.get(..length).unwrap_or_default();
        let answer = handler.respond(request, response).await;
        deliver(input, answer, bootloader).await
    }
}

async fn deliver(
    input: &mut impl EndpointIn,
    answer: Answer<'_>,
    bootloader: &mut (impl UpdateMode + Restart),
) -> Result<(), EndpointError> {
    let written = match answer.reply {
        Some(reply) => input.write_transfer(reply, false).await,
        None => Ok(()),
    };
    match answer.handover {
        Some(Handover::UpdateMode(option)) => bootloader.enter_update_mode(option),
        Some(Handover::Restart(option)) => bootloader.restart(option),
        None => {}
    }
    written
}
