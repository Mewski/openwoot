use core::pin::pin;

use embassy_futures::select::{Either, Either3, select, select3};
use embassy_usb::driver::{EndpointError, EndpointIn, EndpointOut};
use embedded_hal_async::delay::DelayNs;
use openwoot_protocol::report;

use super::{Answer, Configuration, Handler, Handover, Restart, Untimed, UpdateMode};
use crate::usb::{CHANNEL_MAX_PACKET_SIZE, FeatureRequests};

const REQUEST_LEN: usize = report::MAX_LEN.next_multiple_of(CHANNEL_MAX_PACKET_SIZE as usize);
const REPORT_ID_LEN: usize = 1;
const FIRST_REPORT_ID: usize = 1;
const PACKET_GAP_MS: u32 = 100;
const HANDOVER_PAUSE_MS: u32 = 50;

pub struct Channel<Output, Input, Delay = Untimed> {
    output: Output,
    input: Input,
    delay: Option<Delay>,
    request: [u8; REQUEST_LEN],
    response: [u8; report::MAX_LEN],
}

impl<Output: EndpointOut, Input: EndpointIn> Channel<Output, Input> {
    pub const fn new(output: Output, input: Input) -> Self {
        Self {
            output,
            input,
            delay: None,
            request: [0; REQUEST_LEN],
            response: [0; report::MAX_LEN],
        }
    }
}

impl<Output: EndpointOut, Input: EndpointIn, Delay: DelayNs + Clone> Channel<Output, Input, Delay> {
    pub const fn with_delay(output: Output, input: Input, delay: Delay) -> Self {
        Self {
            output,
            input,
            delay: Some(delay),
            request: [0; REQUEST_LEN],
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
            delay,
            request,
            response,
        } = self;
        let length = {
            let mut read = pin!(receive(output, request, delay.clone()));
            loop {
                let event =
                    select3(requests.session_ended(), read.as_mut(), requests.receive()).await;
                match event {
                    Either3::First(()) => handler.end_session(delay.as_mut()).await,
                    Either3::Second(length) => break length?,
                    Either3::Third(feature_request) => {
                        let answer = handler
                            .answer_stock(feature_request, response, delay.as_mut())
                            .await;
                        deliver(input, answer, bootloader, delay.as_mut()).await?;
                    }
                }
            }
        };
        let request = request.get(..length).unwrap_or_default();
        let answer = handler
            .respond_within(request, response, delay.as_mut())
            .await;
        deliver(input, answer, bootloader, delay.as_mut()).await
    }
}

async fn receive(
    output: &mut impl EndpointOut,
    request: &mut [u8],
    mut delay: Option<impl DelayNs>,
) -> Result<usize, EndpointError> {
    let packet_len = usize::from(output.info().max_packet_size);
    let mut length = 0;
    loop {
        let Some(free) = request.get_mut(length..).filter(|free| !free.is_empty()) else {
            return Ok(length);
        };
        let packet = output.read(free);
        let received = match delay.as_mut().filter(|_| length > 0) {
            Some(delay) => match select(packet, delay.delay_ms(PACKET_GAP_MS)).await {
                Either::First(received) => received?,
                Either::Second(()) => {
                    length = 0;
                    continue;
                }
            },
            None => packet.await?,
        };
        length += received;
        if received < packet_len || declared_len(request).is_some_and(|declared| length >= declared)
        {
            return Ok(length);
        }
    }
}

fn declared_len(request: &[u8]) -> Option<usize> {
    let report_id = usize::from(*request.first()?);
    let size = report::SIZES.get(report_id.checked_sub(FIRST_REPORT_ID)?)?;
    Some(REPORT_ID_LEN + size)
}

async fn deliver(
    input: &mut impl EndpointIn,
    answer: Answer<'_>,
    bootloader: &mut (impl UpdateMode + Restart),
    delay: Option<&mut impl DelayNs>,
) -> Result<(), EndpointError> {
    if let Some(reply) = answer.reply {
        input.write_transfer(reply, false).await?;
    }
    let Some(handover) = answer.handover else {
        return Ok(());
    };
    if let Some(delay) = delay {
        delay.delay_ms(HANDOVER_PAUSE_MS).await;
    }
    match handover {
        Handover::UpdateMode(option) => bootloader.enter_update_mode(option),
        Handover::Restart(option) => bootloader.restart(option),
    }
    Ok(())
}
