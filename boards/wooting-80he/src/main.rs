#![no_std]
#![no_main]

use embassy_usb::UsbDevice;
use openwoot::{
    protocol::{Handler, StockIdentity},
    runtime::{self, Configuration, Reports},
    usb::{HostState, Negotiated, Resources},
};
use static_cell::ConstStaticCell;
use wooting_80he::{Board, DESCRIPTOR, FACE, UsbDriver, entry, hal, identity, serial_number};

type Driver = Negotiated<UsbDriver>;

static RESOURCES: ConstStaticCell<Resources<'static>> = ConstStaticCell::new(Resources::new());
static HOST: HostState = HostState::new();

hal::app_descriptor!(DESCRIPTOR);

#[entry]
fn main() -> ! {
    let Some(peripherals) = hal::pac::Peripherals::take() else {
        hal::system::reset()
    };
    let (board, driver) = Board::new(peripherals);
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
        },
    );
    hal::executor::run(|spawner, _keys| {
        if let Ok(token) = device(usb.device) {
            spawner.spawn(token);
        }
        if let Ok(token) = configuration(usb.configuration, handler, board) {
            spawner.spawn(token);
        }
        if let Ok(token) = reports(usb.reports) {
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

#[panic_handler]
fn halt(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
