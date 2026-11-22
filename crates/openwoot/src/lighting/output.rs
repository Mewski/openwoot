use core::future::Future;

use embedded_hal::digital::OutputPin;
use embedded_hal_async::i2c::I2c;
use openwoot_drivers::{Is31fl3741, LedConfiguration, LedError, PwmBank};

pub trait LedMatrix {
    type Error;

    fn initialize(&mut self) -> impl Future<Output = Result<(), Self::Error>>;

    fn set_running(&mut self, running: bool) -> impl Future<Output = Result<(), Self::Error>>;

    fn write(
        &mut self,
        bank: u8,
        first_register: u8,
        values: &[u8],
    ) -> impl Future<Output = Result<(), Self::Error>>;
}

#[derive(Debug)]
pub struct Matrix<I2C, Enable> {
    driver: Is31fl3741<I2C, Enable>,
    running: LedConfiguration,
}

impl<I2C: I2c, Enable: OutputPin> Matrix<I2C, Enable> {
    #[must_use]
    pub const fn new(driver: Is31fl3741<I2C, Enable>, running: LedConfiguration) -> Self {
        Self { driver, running }
    }

    pub fn release(self) -> Is31fl3741<I2C, Enable> {
        self.driver
    }
}

impl<I2C: I2c, Enable: OutputPin> LedMatrix for Matrix<I2C, Enable> {
    type Error = LedError<I2C::Error, Enable::Error>;

    async fn initialize(&mut self) -> Result<(), Self::Error> {
        self.driver.initialize().await
    }

    async fn set_running(&mut self, running: bool) -> Result<(), Self::Error> {
        if running {
            self.driver.run(self.running).await
        } else {
            self.driver.shut_down().await
        }
    }

    async fn write(
        &mut self,
        bank: u8,
        first_register: u8,
        values: &[u8],
    ) -> Result<(), Self::Error> {
        let bank = match bank {
            0 => PwmBank::First,
            1 => PwmBank::Second,
            _ => {
                return Err(LedError::OutsidePage {
                    page: bank,
                    first: usize::from(first_register),
                    last: usize::from(first_register) + values.len().saturating_sub(1),
                });
            }
        };
        self.driver.write_pwm(bank, first_register, values).await
    }
}
