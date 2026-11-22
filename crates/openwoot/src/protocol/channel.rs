use embassy_usb::driver::{EndpointError, EndpointIn, EndpointOut};
use openwoot_protocol::MAX_REPORT_LEN;

use super::Handler;

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

    pub async fn run(&mut self, handler: &mut Handler) -> ! {
        loop {
            self.output.wait_enabled().await;
            while self.serve(handler).await != Err(EndpointError::Disabled) {}
        }
    }

    pub async fn serve(&mut self, handler: &mut Handler) -> Result<(), EndpointError> {
        let len = self.output.read_transfer(&mut self.request).await?;
        let request = self.request.get(..len).unwrap_or_default();
        match handler.respond(request, &mut self.response) {
            Some(response) => self.input.write_transfer(response, false).await,
            None => Ok(()),
        }
    }
}
