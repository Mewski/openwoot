#![no_std]
#![no_main]

use embassy_executor::{SendSpawner, Spawner};
use embassy_futures::block_on;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_usb::UsbDevice;
use openwoot::{
    keys::{Keys, Travels},
    layout::Standard,
    lighting::Bar,
    profile::{Configurator, Model},
    protocol::{self, Handler},
    runtime::{Channel, Firmware, KeyReports, KeyTask, Reports, State},
    usb::Negotiated,
};
use static_cell::StaticCell;
use wooting_80he::{
    Board, DESCRIPTOR, KEY_COLUMNS, KEY_ROWS, UsbDriver, entry, hal,
    keys::{self, PROFILES},
    lighting::{self, COLUMNS, LedDriver, PAGE_LENGTH, PAGES, ROWS, Task},
    scan, storage,
};

type Driver = Negotiated<UsbDriver>;
type Mutual = CriticalSectionRawMutex;
type ProfileModel = Model<'static, Mutual, storage::Nor, KEY_ROWS, KEY_COLUMNS>;
type ProfileConfigurator = Configurator<'static, Mutual, storage::Nor, KEY_ROWS, KEY_COLUMNS>;

static STATE: State<Mutual, Bar, ROWS, COLUMNS, PAGES, PAGE_LENGTH> = State::new();
static MODEL: StaticCell<Mutex<Mutual, ProfileModel>> = StaticCell::new();

hal::app_descriptor!(DESCRIPTOR);

#[entry]
fn main() -> ! {
    hal::executor::run(start)
}

#[inline(never)]
fn start(spawner: Spawner, key_executor: SendSpawner) {
    let Some(peripherals) = hal::pac::Peripherals::take() else {
        hal::system::reset()
    };
    let mut board = Board::new(peripherals);
    let identity = board.identity();
    let variant = board.storage.layout_variant;
    let model = open_model(&mut board);
    let handler = Handler::with_configuration(
        protocol::version(
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        ),
        board.stock_identity(),
        Configurator::new(model, keys::capabilities(board.unit.edition)),
    )
    .with_diagnostics(STATE.readout(hal::usb::negotiated_speed, hal::diagnostics::hardware))
    .with_host(STATE.host())
    .with_host_control(STATE.host_control())
    .with_sensors(STATE.sensors());
    let profile_keys = Configurator::new(model, keys::capabilities(board.unit.edition));
    let face = board.face;
    let firmware = Firmware::new(
        &STATE,
        board.usb,
        hal::usb::negotiated_speed,
        face,
        identity,
        lighting::lighting(variant),
    );
    let travels = Travels {
        travel: scan::SCAN.travel(),
        map: keys::sensor_map(variant),
        shared: STATE.keys(),
        mode: face.mode,
    };
    let leds = board.leds;
    let bootloader = board.bootloader;
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
}

#[inline(never)]
fn open_model(board: &mut Board) -> &'static Mutex<Mutual, ProfileModel> {
    let model = MODEL.init(Mutex::new(Model::new(
        STATE.keys(),
        STATE.profile(),
        board
            .storage
            .settings
            .take()
            .map(|settings| settings.with_diagnostics(STATE.diagnostics())),
        board.unit.factory,
        board.storage.layout_variant,
        PROFILES,
    )));
    block_on(model.get_mut().load());
    model
}

#[embassy_executor::task]
async fn device(mut device: UsbDevice<'static, Driver>) {
    device.run().await;
}

#[embassy_executor::task]
async fn channel(
    channel: Channel<'static, Driver>,
    handler: Handler<ProfileConfigurator>,
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
async fn profiles(profile_keys: ProfileConfigurator) {
    profile_keys.serve().await;
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
