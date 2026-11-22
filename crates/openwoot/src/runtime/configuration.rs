use embassy_usb::driver::Driver;

use crate::{
    Keyboard,
    protocol::{self, Channel, Handler},
    usb::{ChannelEndpoints, HostState},
};

pub struct Configuration<'d, D: Driver<'d>> {
    pub(super) endpoints: ChannelEndpoints<'d, D>,
    pub(super) host: &'d HostState,
}

impl<'d, D: Driver<'d>> Configuration<'d, D> {
    pub async fn run<Board: Keyboard, C: protocol::Configuration>(
        self,
        mut handler: Handler<C>,
        keyboard: &mut Board,
    ) -> ! {
        let mut channel = Channel::new(self.endpoints.output, self.endpoints.input);
        channel
            .run(&mut handler, self.host.feature_requests(), keyboard)
            .await
    }
}
