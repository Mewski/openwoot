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
pub enum LedPage {
    Pwm0,
    Pwm1,
    Scaling0,
    Scaling1,
    Function,
}

impl LedPage {
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
    pub const fn page(self) -> LedPage {
        match self {
            Self::First => LedPage::Pwm0,
            Self::Second => LedPage::Pwm1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LedConfiguration(u8);

impl LedConfiguration {
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
pub enum LedError<BusError: core::fmt::Debug, PinError: core::fmt::Debug> {
    #[error("could not write to the LED driver: {0:?}")]
    Bus(BusError),
    #[error("could not drive the enable line of the LED driver: {0:?}")]
    Enable(PinError),
    #[error("registers {first} to {last} lie outside page {page}")]
    OutsidePage { page: u8, first: usize, last: usize },
}

#[derive(Debug)]
pub struct Is31fl3741<I2C, Enable> {
    bus: I2C,
    enable: Enable,
    address: SevenBitAddress,
    page: Option<LedPage>,
    buffer: [u8; LONGEST_PAGE + 1],
}

impl<I2C: I2c, Enable: OutputPin> Is31fl3741<I2C, Enable> {
    #[must_use]
    pub const fn new(bus: I2C, enable: Enable, address: SevenBitAddress) -> Self {
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
    pub const fn page(&self) -> Option<LedPage> {
        self.page
    }

    pub async fn initialize(&mut self) -> Result<(), LedError<I2C::Error, Enable::Error>> {
        self.write_register(LedPage::Function, FUNCTION_RESET, RESET_KEY)
            .await?;
        self.page = None;
        self.write_register(LedPage::Function, FUNCTION_CONFIGURATION, SHUTDOWN)
            .await?;
        self.enable.set_high().map_err(LedError::Enable)?;
        for page in [LedPage::Scaling0, LedPage::Scaling1] {
            for register in 0..page.register_count() {
                self.write_register(page, register_number(register), SCALING_FULL)
                    .await?;
            }
        }
        self.write_register(
            LedPage::Function,
            FUNCTION_GLOBAL_CURRENT,
            GLOBAL_CURRENT_FULL,
        )
        .await?;
        self.write_register(LedPage::Function, FUNCTION_PULL_UP_DOWN, PULL_UP_DOWN)
            .await
    }

    pub async fn run(
        &mut self,
        configuration: LedConfiguration,
    ) -> Result<(), LedError<I2C::Error, Enable::Error>> {
        self.write_register(
            LedPage::Function,
            FUNCTION_CONFIGURATION,
            configuration.bits(),
        )
        .await
    }

    pub async fn shut_down(&mut self) -> Result<(), LedError<I2C::Error, Enable::Error>> {
        self.write_register(LedPage::Function, FUNCTION_CONFIGURATION, SHUTDOWN)
            .await
    }

    pub async fn write_pwm(
        &mut self,
        bank: PwmBank,
        first: u8,
        values: &[u8],
    ) -> Result<(), LedError<I2C::Error, Enable::Error>> {
        self.write_registers(bank.page(), first, values).await
    }

    pub async fn write_registers(
        &mut self,
        page: LedPage,
        first: u8,
        values: &[u8],
    ) -> Result<(), LedError<I2C::Error, Enable::Error>> {
        let start = usize::from(first);
        let end = start + values.len();
        let outside = LedError::OutsidePage {
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
            .map_err(LedError::Bus)
    }

    pub fn release(self) -> (I2C, Enable) {
        (self.bus, self.enable)
    }

    async fn write_register(
        &mut self,
        page: LedPage,
        register: u8,
        value: u8,
    ) -> Result<(), LedError<I2C::Error, Enable::Error>> {
        self.write_registers(page, register, &[value]).await
    }

    async fn select(&mut self, page: LedPage) -> Result<(), LedError<I2C::Error, Enable::Error>> {
        if self.page == Some(page) {
            return Ok(());
        }
        self.page = None;
        self.bus
            .write(self.address, &[COMMAND_UNLOCK, UNLOCK_KEY])
            .await
            .map_err(LedError::Bus)?;
        self.bus
            .write(self.address, &[COMMAND, page.number()])
            .await
            .map_err(LedError::Bus)?;
        self.page = Some(page);
        Ok(())
    }
}

fn register_number(register: usize) -> u8 {
    u8::try_from(register).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use core::{cell::RefCell, convert::Infallible};
    use std::{rc::Rc, vec, vec::Vec};

    use embassy_futures::block_on;
    use embedded_hal_async::i2c::{ErrorKind, ErrorType, Operation};

    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Event {
        Write(SevenBitAddress, Vec<u8>),
        Enable,
    }

    #[derive(Clone, Default)]
    struct Bus {
        log: Rc<RefCell<Vec<Event>>>,
        failing: bool,
    }

    impl ErrorType for Bus {
        type Error = ErrorKind;
    }

    impl I2c for Bus {
        #[expect(
            clippy::unused_async_trait_impl,
            reason = "the bus model completes every transfer at once"
        )]
        async fn transaction(
            &mut self,
            address: SevenBitAddress,
            operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            if self.failing {
                return Err(ErrorKind::NoAcknowledge(
                    embedded_hal_async::i2c::NoAcknowledgeSource::Address,
                ));
            }
            for operation in operations {
                if let Operation::Write(bytes) = operation {
                    self.log
                        .borrow_mut()
                        .push(Event::Write(address, bytes.to_vec()));
                }
            }
            Ok(())
        }
    }

    struct EnableLine(Rc<RefCell<Vec<Event>>>);

    impl embedded_hal::digital::ErrorType for EnableLine {
        type Error = Infallible;
    }

    impl OutputPin for EnableLine {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.0.borrow_mut().push(Event::Enable);
            Ok(())
        }
    }

    fn driver() -> (Is31fl3741<Bus, EnableLine>, Rc<RefCell<Vec<Event>>>) {
        let bus = Bus::default();
        let log = bus.log.clone();
        (Is31fl3741::new(bus, EnableLine(log.clone()), 0x30), log)
    }

    fn write(bytes: &[u8]) -> Event {
        Event::Write(0x30, bytes.to_vec())
    }

    fn select(page: u8) -> [Event; 2] {
        [write(&[0xFE, 0xC5]), write(&[0xFD, page])]
    }

    #[test]
    fn initialization_resets_shuts_down_enables_scales_and_sets_the_current() {
        let (mut driver, log) = driver();
        block_on(driver.initialize()).unwrap();
        let mut expected = Vec::new();
        expected.extend(select(4));
        expected.push(write(&[0x3F, 0xAE]));
        expected.extend(select(4));
        expected.push(write(&[0x00, 0x00]));
        expected.push(Event::Enable);
        expected.extend(select(2));
        expected.extend((0..180).map(|register| write(&[register, 0xFF])));
        expected.extend(select(3));
        expected.extend((0..171).map(|register| write(&[register, 0xFF])));
        expected.extend(select(4));
        expected.push(write(&[0x01, 0xFF]));
        expected.push(write(&[0x02, 0x35]));
        assert_eq!(*log.borrow(), expected);
        assert_eq!(driver.page(), Some(LedPage::Function));
    }

    #[test]
    fn writes_to_the_selected_page_skip_the_unlock_and_select() {
        let (mut driver, log) = driver();
        block_on(driver.write_pwm(PwmBank::First, 30, &[1, 2, 3])).unwrap();
        block_on(driver.write_pwm(PwmBank::First, 60, &[4])).unwrap();
        block_on(driver.write_pwm(PwmBank::Second, 90, &[5, 6])).unwrap();
        block_on(driver.run(LedConfiguration::new(0x11))).unwrap();
        block_on(driver.shut_down()).unwrap();
        let mut expected = Vec::new();
        expected.extend(select(0));
        expected.push(write(&[30, 1, 2, 3]));
        expected.push(write(&[60, 4]));
        expected.extend(select(1));
        expected.push(write(&[90, 5, 6]));
        expected.extend(select(4));
        expected.push(write(&[0x00, 0x11]));
        expected.push(write(&[0x00, 0x00]));
        assert_eq!(*log.borrow(), expected);
    }

    #[test]
    fn a_page_run_goes_out_in_one_transfer() {
        let (mut driver, log) = driver();
        let values: Vec<u8> = (0..36).collect();
        block_on(driver.write_pwm(PwmBank::Second, 126, &values)).unwrap();
        let mut transfer = vec![126];
        transfer.extend(&values);
        assert_eq!(log.borrow().last(), Some(&write(&transfer)));
    }

    #[test]
    fn writes_past_the_page_end_are_refused_without_bus_traffic() {
        let (mut driver, log) = driver();
        let error = block_on(driver.write_pwm(PwmBank::Second, 170, &[1, 2])).unwrap_err();
        assert!(matches!(
            error,
            LedError::OutsidePage {
                page: 1,
                first: 170,
                last: 171
            }
        ));
        assert!(block_on(driver.write_pwm(PwmBank::First, 0, &[])).is_err());
        assert!(log.borrow().is_empty());
    }

    #[test]
    fn a_failed_select_forgets_the_page() {
        let (mut driver, log) = driver();
        block_on(driver.write_pwm(PwmBank::First, 0, &[1])).unwrap();
        driver.bus.failing = true;
        assert!(matches!(
            block_on(driver.write_pwm(PwmBank::Second, 0, &[1])),
            Err(LedError::Bus(_))
        ));
        assert_eq!(driver.page(), None);
        driver.bus.failing = false;
        log.borrow_mut().clear();
        block_on(driver.write_pwm(PwmBank::First, 0, &[2])).unwrap();
        let mut expected = Vec::new();
        expected.extend(select(0));
        expected.push(write(&[0, 2]));
        assert_eq!(*log.borrow(), expected);
    }
}
