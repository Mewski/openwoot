use core::pin::pin;

use embassy_futures::select::{Either, select};
use embassy_usb::driver::{EndpointError, EndpointIn, EndpointOut};
use openwoot_protocol::report;

use super::{Answer, Configuration, Handler, UpdateMode};
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
        update_mode: &mut impl UpdateMode,
    ) -> ! {
        loop {
            self.output.wait_enabled().await;
            while self.serve(handler, requests, update_mode).await != Err(EndpointError::Disabled) {
            }
        }
    }

    pub async fn serve<C: Configuration>(
        &mut self,
        handler: &mut Handler<C>,
        requests: &FeatureRequests,
        update_mode: &mut impl UpdateMode,
    ) -> Result<(), EndpointError> {
        let Self {
            output,
            input,
            request,
            response,
        } = self;
        let len = {
            let mut read = pin!(output.read_transfer(request));
            loop {
                match select(read.as_mut(), requests.receive()).await {
                    Either::First(len) => break len?,
                    Either::Second(feature_request) => {
                        let answer = handler.answer_stock(feature_request, response);
                        deliver(input, answer, update_mode).await?;
                    }
                }
            }
        };
        let request = request.get(..len).unwrap_or_default();
        let answer = handler.respond(request, response).await;
        deliver(input, answer, update_mode).await
    }
}

async fn deliver(
    input: &mut impl EndpointIn,
    answer: Answer<'_>,
    update_mode: &mut impl UpdateMode,
) -> Result<(), EndpointError> {
    let written = match answer.reply {
        Some(reply) => input.write_transfer(reply, false).await,
        None => Ok(()),
    };
    if let Some(option) = answer.update_mode {
        update_mode.enter_update_mode(option);
    }
    written
}
