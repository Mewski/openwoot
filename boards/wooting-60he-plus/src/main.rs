#![no_std]
#![no_main]

use embassy_usb::UsbDevice;
use openwoot::{
    lighting::KeyEvents,
    protocol::{Handler, StockIdentity},
    runtime::{self, Configuration, Reports},
    usb::{HostState, Negotiated, Resources},
};
use static_cell::{ConstStaticCell, StaticCell};
use wooting_60he_plus::{
    Board, DESCRIPTOR, FACE, UsbDriver, entry, hal, identity,
    lighting::{self, LedDriver, SharedLighting, Task},
    scan, serial_number,
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
    let (mut board, driver) = Board::new(peripherals);
    scan::select_layout(board.storage.layout_variant);
    let serial_number = serial_number();
    let usb = runtime::Usb::new(
        driver,
        hal::usb::negotiated_speed,
        FACE,
        identity(serial_number),
        RESOURCES.take(),
        &HOST,
    );
    let handler = Handler::new(
        runtime::version(
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        ),
        StockIdentity {
            serial: serial_number,
            face: FACE,
            layout: board.storage.layout_variant,
            mcu_id: board.storage.mcu_id,
            flash_ids: board.storage.flash_ids.to_bytes(),
        },
    );
    let leds = board.leds.take();
    let shared = LIGHTING.init(SharedLighting::new(lighting::lighting(
        board.storage.layout_variant,
    )));
    hal::executor::run(|spawner, keys| {
        if let Ok(token) = scanning() {
            keys.spawn(token);
        }
        if let Ok(token) = device(usb.device) {
            spawner.spawn(token);
        }
        if let Ok(token) = configuration(usb.configuration, handler, board) {
            spawner.spawn(token);
        }
        if let Ok(token) = reports(usb.reports) {
            spawner.spawn(token);
        }
        if let Some(leds) = leds
            && let Ok(token) = lights(
                Task {
                    lighting: shared,
                    events: &KEY_EVENTS,
                    host: &HOST,
                },
                leds,
            )
        {
            spawner.spawn(token);
        }
    })
}

#[embassy_executor::task]
async fn device(mut device: UsbDevice<'static, Driver>) {
    device.run().await;
}

#[embassy_executor::task]
async fn configuration(
    configuration: Configuration<'static, Driver>,
    handler: Handler,
    mut board: Board,
) {
    configuration.run(handler, &mut board).await;
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
