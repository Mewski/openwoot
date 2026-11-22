use embassy_usb::driver::Driver;

use crate::{
    protocol::{self, Configuration, Handler, Restart, UpdateMode},
    usb::{ChannelEndpoints, HostState},
};

pub struct Channel<'d, D: Driver<'d>> {
    pub(super) endpoints: ChannelEndpoints<'d, D>,
    pub(super) host: &'d HostState,
}

impl<'d, D: Driver<'d>> Channel<'d, D> {
    pub async fn run<C: Configuration>(
        self,
        mut handler: Handler<C>,
        bootloader: &mut (impl UpdateMode + Restart),
    ) -> ! {
        let mut channel = protocol::Channel::new(self.endpoints.output, self.endpoints.input);
        channel
            .run(&mut handler, self.host.feature_requests(), bootloader)
            .await
    }
}
