#![no_std]
#![no_main]

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_usb::UsbDevice;
use openwoot::{
    protocol::{self, Handler},
    runtime::{Channel, Firmware, Reports, State},
    usb::Negotiated,
};
use wooting_60he_plus::{
    Board, DESCRIPTOR, FACE, UsbDriver, entry, hal,
    lighting::{self, COLUMNS, LedDriver, PAGE_LENGTH, PAGES, ROWS, Task},
    scan,
};

type Driver = Negotiated<UsbDriver>;

static STATE: State<CriticalSectionRawMutex, (), ROWS, COLUMNS, PAGES, PAGE_LENGTH> = State::new();

hal::app_descriptor!(DESCRIPTOR);

#[entry]
fn main() -> ! {
    let Some(peripherals) = hal::pac::Peripherals::take() else {
        hal::system::reset()
    };
    let board = Board::new(peripherals);
    let identity = board.identity();
    let handler = Handler::new(
        protocol::version(
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        ),
        board.stock_identity(),
    );
    let firmware = Firmware::new(
        &STATE,
        board.usb,
        hal::usb::negotiated_speed,
        FACE,
        identity,
        lighting::lighting(board.storage.layout_variant),
    );
    let leds = board.leds;
    let bootloader = board.bootloader;
    hal::executor::run(|spawner, keys| {
        if let Ok(token) = scanning() {
            keys.spawn(token);
        }
        if let Ok(token) = device(firmware.device) {
            spawner.spawn(token);
        }
        if let Ok(token) = channel(firmware.channel, handler, bootloader) {
            spawner.spawn(token);
        }
        if let Ok(token) = reports(firmware.reports) {
            spawner.spawn(token);
        }
        if let Ok(token) = lights(firmware.lighting, leds) {
            spawner.spawn(token);
        }
    })
}

#[embassy_executor::task]
async fn device(mut device: UsbDevice<'static, Driver>) {
    device.run().await;
}

#[embassy_executor::task]
async fn channel(
    channel: Channel<'static, Driver>,
    handler: Handler,
    mut bootloader: hal::system::Bootloader,
) {
    channel.run(handler, &mut bootloader).await;
}

#[embassy_executor::task]
async fn reports(reports: Reports<'static, Driver>) {
    reports.run().await;
}

#[embassy_executor::task]
async fn lights(task: Task, mut leds: LedDriver) {
    task.run(&mut leds, &()).await;
}

#[embassy_executor::task]
async fn scanning() {
    scan::SCAN.run(|_| {}).await;
}

#[panic_handler]
fn halt(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
