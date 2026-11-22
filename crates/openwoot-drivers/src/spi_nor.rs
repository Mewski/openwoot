use embedded_hal_async::spi::SpiDevice;

pub struct SpiNor<SPI> {
    device: SPI,
}

impl<SPI: SpiDevice> SpiNor<SPI> {
    #[must_use]
    pub const fn new(device: SPI) -> Self {
        Self { device }
    }

    pub fn release(self) -> SPI {
        self.device
    }
}
