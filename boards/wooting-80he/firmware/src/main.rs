#![no_std]
#![no_main]

use embassy_executor::SendSpawner;
use openwoot::{
    keys::Keys,
    layout::Standard,
    protocol,
    runtime::{KeyReports, KeyTask, Reports, UsbParts, UsbTasks},
};
use wooting_80he::{
    Board, DESCRIPTOR, KEY_COLUMNS, KEY_ROWS, UsbDriver,
    configuration::{self, Driver, Mutual},
    entry, hal,
    hal::executor::Spawners,
    scan,
};

hal::bind_interrupts!(struct Irqs {
    USBHS => hal::usb::InterruptHandler;
    DMA1_Channel2 => hal::spi::InterruptHandler;
    I2C0_EV => hal::i2c::InterruptHandler;
    I2C0_ER => hal::i2c::InterruptHandler;
    DMA0_Channel6 => hal::i2c::InterruptHandler;
    ADC => hal::scan::InterruptHandler<scan::Interrupt>;
});

hal::app_descriptor!(DESCRIPTOR);

#[entry]
fn main() -> ! {
    hal::executor::run(start)
}

#[inline(never)]
fn start(spawners: Spawners) {
    let Some(peripherals) = hal::pac::Peripherals::take() else {
        hal::system::reset()
    };
    let tasks = configuration::start(
        Board::new(peripherals, &Irqs),
        protocol::version(
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        ),
    );
    if let Ok(token) = scanning(tasks.keys) {
        spawners.keys.spawn(token);
    }
    if let Ok(token) = device(tasks.usb, tasks.key_reports, spawners.device) {
        spawners.device.spawn(token);
    }
    if let Ok(token) = configuration::lights(tasks.lighting, tasks.leds, tasks.travels) {
        spawners.lighting.spawn(token);
    }
    if let Ok(token) = configuration::channel(tasks.handler, tasks.bootloader) {
        spawners.thread.spawn(token);
    }
    if let Ok(token) = configuration::profiles(tasks.profile_keys) {
        spawners.thread.spawn(token);
    }
}

#[embassy_executor::task]
async fn scanning(task: KeyTask<'static, Mutual>) {
    let mut keys = Keys::<_, KEY_ROWS, KEY_COLUMNS>::new(Standard);
    task.run(&scan::SCAN, &mut keys).await;
}

#[embassy_executor::task]
async fn device(
    usb: UsbParts<'static, UsbDriver, Mutual>,
    key_reports: &'static KeyReports<Mutual>,
    spawner: SendSpawner,
) {
    let UsbTasks {
        mut device,
        channel,
        reports: usb_reports,
    } = usb.build();
    configuration::hand_over(channel);
    if let Ok(token) = reports(usb_reports, key_reports) {
        spawner.spawn(token);
    }
    device.run().await;
}

#[embassy_executor::task]
async fn reports(reports: Reports<'static, Driver>, key_reports: &'static KeyReports<Mutual>) {
    reports.run(key_reports).await;
}

#[panic_handler]
fn restart(_info: &core::panic::PanicInfo) -> ! {
    hal::system::reset()
}
