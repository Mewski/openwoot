use embedded_hal_async::i2c::{I2c, SevenBitAddress};

pub struct Is31fl3741<I2C> {
    bus: I2C,
    address: SevenBitAddress,
}

impl<I2C: I2c> Is31fl3741<I2C> {
    #[must_use]
    pub const fn new(bus: I2C, address: SevenBitAddress) -> Self {
        Self { bus, address }
    }

    #[must_use]
    pub const fn address(&self) -> SevenBitAddress {
        self.address
    }

    pub fn release(self) -> I2C {
        self.bus
    }
}
