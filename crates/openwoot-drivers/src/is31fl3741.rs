use embedded_hal::digital::OutputPin;
use embedded_hal_async::i2c::{I2c, SevenBitAddress};

const COMMAND: u8 = 0xFD;
const COMMAND_UNLOCK: u8 = 0xFE;
const UNLOCK_KEY: u8 = 0xC5;
const FUNCTION_CONFIGURATION: u8 = 0x00;
const FUNCTION_GLOBAL_CURRENT: u8 = 0x01;
const FUNCTION_PULL_UP_DOWN: u8 = 0x02;
const FUNCTION_RESET: u8 = 0x3F;
const RESET_KEY: u8 = 0xAE;
const SHUTDOWN: u8 = 0x00;
const SCALING_FULL: u8 = 0xFF;
const GLOBAL_CURRENT_FULL: u8 = 0xFF;
const PULL_UP_DOWN: u8 = 0x35;
const LONGEST_PAGE: usize = 180;
const SHORTER_PAGE: usize = 171;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Pwm0,
    Pwm1,
    Scaling0,
    Scaling1,
    Function,
}

impl Page {
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Pwm0 => 0,
            Self::Pwm1 => 1,
            Self::Scaling0 => 2,
            Self::Scaling1 => 3,
            Self::Function => 4,
        }
    }

    #[must_use]
    pub const fn register_count(self) -> usize {
        match self {
            Self::Pwm0 | Self::Scaling0 => LONGEST_PAGE,
            Self::Pwm1 | Self::Scaling1 => SHORTER_PAGE,
            Self::Function => FUNCTION_RESET as usize + 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PwmBank {
    First,
    Second,
}

impl PwmBank {
    #[must_use]
    pub const fn page(self) -> Page {
        match self {
            Self::First => Page::Pwm0,
            Self::Second => Page::Pwm1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configuration(u8);

impl Configuration {
    #[must_use]
    pub const fn new(bits: u8) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error<BusError: core::fmt::Debug, PinError: core::fmt::Debug> {
    #[error("could not write to the LED driver: {0:?}")]
    Bus(BusError),
    #[error("could not drive the enable line of the LED driver: {0:?}")]
    Enable(PinError),
    #[error("registers {first} to {last} lie outside page {page}")]
    OutsidePage { page: u8, first: usize, last: usize },
}

#[derive(Debug)]
pub struct Is31fl3741<Bus, Enable> {
    bus: Bus,
    enable: Enable,
    address: SevenBitAddress,
    page: Option<Page>,
    buffer: [u8; LONGEST_PAGE + 1],
}

impl<Bus: I2c, Enable: OutputPin> Is31fl3741<Bus, Enable> {
    #[must_use]
    pub const fn new(bus: Bus, enable: Enable, address: SevenBitAddress) -> Self {
        Self {
            bus,
            enable,
            address,
            page: None,
            buffer: [0; LONGEST_PAGE + 1],
        }
    }

    #[must_use]
    pub const fn address(&self) -> SevenBitAddress {
        self.address
    }

    #[must_use]
    pub const fn page(&self) -> Option<Page> {
        self.page
    }

    pub async fn initialize(&mut self) -> Result<(), Error<Bus::Error, Enable::Error>> {
        self.write_register(Page::Function, FUNCTION_RESET, RESET_KEY)
            .await?;
        self.page = None;
        self.write_register(Page::Function, FUNCTION_CONFIGURATION, SHUTDOWN)
            .await?;
        self.enable.set_high().map_err(Error::Enable)?;
        for page in [Page::Scaling0, Page::Scaling1] {
            for register in 0..page.register_count() {
                self.write_register(page, register_number(register), SCALING_FULL)
                    .await?;
            }
        }
        self.write_register(Page::Function, FUNCTION_GLOBAL_CURRENT, GLOBAL_CURRENT_FULL)
            .await?;
        self.write_register(Page::Function, FUNCTION_PULL_UP_DOWN, PULL_UP_DOWN)
            .await
    }

    pub async fn run(
        &mut self,
        configuration: Configuration,
    ) -> Result<(), Error<Bus::Error, Enable::Error>> {
        self.write_register(Page::Function, FUNCTION_CONFIGURATION, configuration.bits())
            .await
    }

    pub async fn shut_down(&mut self) -> Result<(), Error<Bus::Error, Enable::Error>> {
        self.write_register(Page::Function, FUNCTION_CONFIGURATION, SHUTDOWN)
            .await
    }

    pub async fn write_pwm(
        &mut self,
        bank: PwmBank,
        first: u8,
        values: &[u8],
    ) -> Result<(), Error<Bus::Error, Enable::Error>> {
        self.write_registers(bank.page(), first, values).await
    }

    pub async fn write_registers(
        &mut self,
        page: Page,
        first: u8,
        values: &[u8],
    ) -> Result<(), Error<Bus::Error, Enable::Error>> {
        let start = usize::from(first);
        let end = start + values.len();
        let outside = Error::OutsidePage {
            page: page.number(),
            first: start,
            last: end.saturating_sub(1),
        };
        if values.is_empty() || end > page.register_count() {
            return Err(outside);
        }
        let Some((register, data)) = self.buffer.split_first_mut() else {
            return Err(outside);
        };
        let Some(data) = data.get_mut(..values.len()) else {
            return Err(outside);
        };
        *register = first;
        data.copy_from_slice(values);
        self.select(page).await?;
        let Some(transfer) = self.buffer.get(..=values.len()) else {
            return Err(outside);
        };
        self.bus
            .write(self.address, transfer)
            .await
            .map_err(Error::Bus)
    }

    pub fn release(self) -> (Bus, Enable) {
        (self.bus, self.enable)
    }

    async fn write_register(
        &mut self,
        page: Page,
        register: u8,
        value: u8,
    ) -> Result<(), Error<Bus::Error, Enable::Error>> {
        self.write_registers(page, register, &[value]).await
    }

    async fn select(&mut self, page: Page) -> Result<(), Error<Bus::Error, Enable::Error>> {
        if self.page == Some(page) {
            return Ok(());
        }
        self.page = None;
        self.bus
            .write(self.address, &[COMMAND_UNLOCK, UNLOCK_KEY])
            .await
            .map_err(Error::Bus)?;
        self.bus
            .write(self.address, &[COMMAND, page.number()])
            .await
            .map_err(Error::Bus)?;
        self.page = Some(page);
        Ok(())
    }
}

fn register_number(register: usize) -> u8 {
    u8::try_from(register).unwrap_or(u8::MAX)
}
