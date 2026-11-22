use embassy_usb::driver::Driver;

use crate::{
    Keyboard,
    protocol::{Channel, Handler},
    usb::{ChannelEndpoints, HostState},
};

pub struct Configuration<'d, D: Driver<'d>> {
    pub(super) endpoints: ChannelEndpoints<'d, D>,
    pub(super) host: &'d HostState,
}

impl<'d, D: Driver<'d>> Configuration<'d, D> {
    pub async fn run<Board: Keyboard>(self, mut handler: Handler, keyboard: &mut Board) -> ! {
        let mut channel = Channel::new(self.endpoints.output, self.endpoints.input);
        channel
            .run(&mut handler, self.host.feature_requests(), keyboard)
            .await
    }
}
