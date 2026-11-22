use core::pin::pin;

use embassy_futures::select::{Either, select};
use embassy_usb::driver::{EndpointError, EndpointIn, EndpointOut};
use openwoot_protocol::MAX_REPORT_LEN;

use super::{Answer, Configuration, FeatureRequests, Handler};
use crate::Keyboard;

pub struct Channel<Output, Input> {
    output: Output,
    input: Input,
    request: [u8; MAX_REPORT_LEN],
    response: [u8; MAX_REPORT_LEN],
}

impl<Output: EndpointOut, Input: EndpointIn> Channel<Output, Input> {
    pub const fn new(output: Output, input: Input) -> Self {
        Self {
            output,
            input,
            request: [0; MAX_REPORT_LEN],
            response: [0; MAX_REPORT_LEN],
        }
    }

    pub async fn run<Board: Keyboard, C: Configuration>(
        &mut self,
        handler: &mut Handler<C>,
        requests: &FeatureRequests,
        keyboard: &mut Board,
    ) -> ! {
        loop {
            self.output.wait_enabled().await;
            while self.serve(handler, requests, keyboard).await != Err(EndpointError::Disabled) {}
        }
    }

    pub async fn serve<Board: Keyboard, C: Configuration>(
        &mut self,
        handler: &mut Handler<C>,
        requests: &FeatureRequests,
        keyboard: &mut Board,
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
                    Either::Second(queued) => {
                        let answer = handler.answer_stock(queued, response);
                        deliver(input, answer, keyboard).await?;
                    }
                }
            }
        };
        let request = request.get(..len).unwrap_or_default();
        let answer = handler.respond(request, response).await;
        deliver(input, answer, keyboard).await
    }
}

async fn deliver<Input: EndpointIn, Board: Keyboard>(
    input: &mut Input,
    answer: Answer<'_>,
    keyboard: &mut Board,
) -> Result<(), EndpointError> {
    let written = match answer.reply {
        Some(reply) => input.write_transfer(reply, false).await,
        None => Ok(()),
    };
    if let Some(option) = answer.update_mode {
        keyboard.enter_update_mode(option);
    }
    written
}
