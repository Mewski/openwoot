#![no_std]
#![no_main]

use embassy_usb::UsbDevice;
use openwoot::{
    lighting::KeyEvents,
    protocol::{self, Handler},
    runtime::{self, Channel, Reports},
    usb::{HostState, Negotiated, Resources},
};
use static_cell::{ConstStaticCell, StaticCell};
use wooting_80he::{
    Board, DESCRIPTOR, FACE, UsbDriver, entry, hal,
    lighting::{self, LedDriver, SharedLighting, Task},
    scan,
};

type Driver = Negotiated<UsbDriver>;

static RESOURCES: ConstStaticCell<Resources<'static>> = ConstStaticCell::new(Resources::new());
static HOST: HostState = HostState::new();
static KEY_EVENTS: KeyEvents = KeyEvents::new();
static LIGHTING: StaticCell<SharedLighting> = StaticCell::new();

hal::app_descriptor!(DESCRIPTOR);

#[entry]
fn main() -> ! {
    let Some(peripherals) = hal::pac::Peripherals::take() else {
        hal::system::reset()
    };
    let board = Board::new(peripherals);
    let identity = board.identity();
    let stock_identity = board.stock_identity();
    let lighting = lighting::lighting(board.storage.layout_variant);
    let usb = runtime::Usb::new(
        board.usb,
        hal::usb::negotiated_speed,
        FACE,
        identity,
        RESOURCES.take(),
        &HOST,
    );
    let handler = Handler::new(
        protocol::version(
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        ),
        stock_identity,
    );
    let lights_task = Task {
        lighting: LIGHTING.init(SharedLighting::new(lighting)),
        events: &KEY_EVENTS,
        host: &HOST,
    };
    let leds = board.leds;
    let bootloader = board.bootloader;
    hal::executor::run(|spawner, keys| {
        if let Ok(token) = scanning() {
            keys.spawn(token);
        }
        if let Ok(token) = device(usb.device) {
            spawner.spawn(token);
        }
        if let Ok(token) = channel(usb.channel, handler, bootloader) {
            spawner.spawn(token);
        }
        if let Ok(token) = reports(usb.reports) {
            spawner.spawn(token);
        }
        if let Ok(token) = lights(lights_task, leds) {
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
