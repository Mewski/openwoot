use core::{
    cell::{Cell, RefCell},
    convert::Infallible,
};
use std::rc::Rc;

use embassy_futures::block_on;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::i2c::{
    ErrorKind, ErrorType, I2c, NoAcknowledgeSource, Operation, SevenBitAddress,
};
use openwoot_drivers::is31fl3741::{Configuration, Error, Is31fl3741, Page, PwmBank};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Write(SevenBitAddress, Vec<u8>),
    Enable,
}

type Log = Rc<RefCell<Vec<Event>>>;

#[derive(Clone, Default)]
struct Bus {
    log: Log,
    failing: Rc<Cell<bool>>,
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
        if self.failing.get() {
            return Err(ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address));
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

struct EnableLine(Log);

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

fn driver() -> (Is31fl3741<Bus, EnableLine>, Bus) {
    let bus = Bus::default();
    let enable = EnableLine(Rc::clone(&bus.log));
    (Is31fl3741::new(bus.clone(), enable, 0x30), bus)
}

fn write(bytes: &[u8]) -> Event {
    Event::Write(0x30, bytes.to_vec())
}

fn select(page: u8) -> [Event; 2] {
    [write(&[0xFE, 0xC5]), write(&[0xFD, page])]
}

#[test]
fn initialization_resets_shuts_down_enables_scales_and_sets_the_current() {
    let (mut driver, bus) = driver();
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
    assert_eq!(*bus.log.borrow(), expected);
    assert_eq!(driver.page(), Some(Page::Function));
}

#[test]
fn writes_to_the_selected_page_skip_the_unlock_and_select() {
    let (mut driver, bus) = driver();
    block_on(driver.write_pwm(PwmBank::First, 30, &[1, 2, 3])).unwrap();
    block_on(driver.write_pwm(PwmBank::First, 60, &[4])).unwrap();
    block_on(driver.write_pwm(PwmBank::Second, 90, &[5, 6])).unwrap();
    block_on(driver.run(Configuration::new(0x11))).unwrap();
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
    assert_eq!(*bus.log.borrow(), expected);
}

#[test]
fn a_page_run_goes_out_in_one_transfer() {
    let (mut driver, bus) = driver();
    let values: Vec<u8> = (0..36).collect();
    block_on(driver.write_pwm(PwmBank::Second, 126, &values)).unwrap();
    let mut transfer = vec![126];
    transfer.extend(&values);
    assert_eq!(bus.log.borrow().last(), Some(&write(&transfer)));
}

#[test]
fn writes_past_the_page_end_are_refused_without_bus_traffic() {
    let (mut driver, bus) = driver();
    let error = block_on(driver.write_pwm(PwmBank::Second, 170, &[1, 2])).unwrap_err();
    assert!(matches!(
        error,
        Error::OutsidePage {
            page: 1,
            first: 170,
            last: 171
        }
    ));
    assert!(block_on(driver.write_pwm(PwmBank::First, 0, &[])).is_err());
    assert!(bus.log.borrow().is_empty());
}

#[test]
fn a_failed_select_forgets_the_page() {
    let (mut driver, bus) = driver();
    block_on(driver.write_pwm(PwmBank::First, 0, &[1])).unwrap();
    bus.failing.set(true);
    assert!(matches!(
        block_on(driver.write_pwm(PwmBank::Second, 0, &[1])),
        Err(Error::Bus(_))
    ));
    assert_eq!(driver.page(), None);
    bus.failing.set(false);
    bus.log.borrow_mut().clear();
    block_on(driver.write_pwm(PwmBank::First, 0, &[2])).unwrap();
    let mut expected = Vec::new();
    expected.extend(select(0));
    expected.push(write(&[0, 2]));
    assert_eq!(*bus.log.borrow(), expected);
}
