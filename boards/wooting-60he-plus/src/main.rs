#![no_std]
#![no_main]

use embassy_futures::block_on;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_usb::UsbDevice;
use openwoot::{
    keys::{Configurator, Keys, Model, Travels},
    layout::Standard,
    protocol::{self, Handler},
    runtime::{Channel, Firmware, KeyReports, KeyTask, Reports, State},
    usb::Negotiated,
};
use static_cell::StaticCell;
use wooting_60he_plus::{
    Board, DESCRIPTOR, FACE, KEY_COLUMNS, KEY_ROWS, UsbDriver, entry, hal,
    keys::{self, KEYMAP, PROFILES},
    lighting::{self, COLUMNS, LedDriver, PAGE_LENGTH, PAGES, ROWS, Task},
    scan, storage,
};

type Driver = Negotiated<UsbDriver>;
type Mutual = CriticalSectionRawMutex;
type KeyModel = Model<'static, Mutual, storage::Nor, KEY_ROWS, KEY_COLUMNS>;
type KeyConfigurator = Configurator<'static, Mutual, storage::Nor, KEY_ROWS, KEY_COLUMNS>;

static STATE: State<Mutual, (), ROWS, COLUMNS, PAGES, PAGE_LENGTH> = State::new();
static MODEL: StaticCell<Mutex<Mutual, KeyModel>> = StaticCell::new();

hal::app_descriptor!(DESCRIPTOR);

#[entry]
fn main() -> ! {
    let Some(peripherals) = hal::pac::Peripherals::take() else {
        hal::system::reset()
    };
    let mut board = Board::new(peripherals);
    let identity = board.identity();
    let variant = board.storage.layout_variant;
    let model = MODEL.init(Mutex::new(block_on(Model::open(
        STATE.keys(),
        board.storage.settings.take(),
        KEYMAP,
        variant,
        PROFILES,
    ))));
    let handler = Handler::with_configuration(
        protocol::version(
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        ),
        board.stock_identity(),
        Configurator::new(model, keys::capabilities()),
    );
    let profile_keys = Configurator::new(model, keys::capabilities());
    let firmware = Firmware::new(
        &STATE,
        board.usb,
        hal::usb::negotiated_speed,
        FACE,
        identity,
        lighting::lighting(variant),
    );
    let travels = Travels {
        travel: scan::SCAN.travel(),
        map: keys::sensor_map(variant),
        shared: STATE.keys(),
        mode: FACE.mode,
    };
    let leds = board.leds;
    let bootloader = board.bootloader;
    hal::executor::run(|spawner, key_executor| {
        if let Ok(token) = scanning(firmware.keys) {
            key_executor.spawn(token);
        }
        if let Ok(token) = device(firmware.device) {
            spawner.spawn(token);
        }
        if let Ok(token) = channel(firmware.channel, handler, bootloader) {
            spawner.spawn(token);
        }
        if let Ok(token) = reports(firmware.reports, firmware.key_reports) {
            spawner.spawn(token);
        }
        if let Ok(token) = lights(firmware.lighting, leds, travels) {
            spawner.spawn(token);
        }
        if let Ok(token) = profiles(profile_keys) {
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
    handler: Handler<KeyConfigurator>,
    mut bootloader: hal::system::Bootloader,
) {
    channel.run(handler, &mut bootloader).await;
}

#[embassy_executor::task]
async fn reports(reports: Reports<'static, Driver>, key_reports: &'static KeyReports<Mutual>) {
    reports.run(key_reports).await;
}

#[embassy_executor::task]
async fn lights(
    task: Task,
    mut leds: LedDriver,
    travels: Travels<'static, Mutual, { scan::INPUTS }, { scan::ADDRESSES }>,
) {
    task.run(&mut leds, &travels).await;
}

#[embassy_executor::task]
async fn profiles(profile_keys: KeyConfigurator) {
    profile_keys.serve_profile_keys().await;
}

#[embassy_executor::task]
async fn scanning(task: KeyTask<'static, Mutual>) {
    let mut keys = Keys::<_, KEY_ROWS, KEY_COLUMNS>::new(Standard);
    task.run(&scan::SCAN, &mut keys).await;
}

#[panic_handler]
fn halt(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
