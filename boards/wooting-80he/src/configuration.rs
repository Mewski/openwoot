use core::cell::RefCell;

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use openwoot::{
    keys::Travels,
    lighting::Bar,
    profile::{Configurator, Model},
    protocol::Handler,
    runtime::{Channel, Firmware, KeyReports, KeyTask, State, UsbParts},
    usb::Negotiated,
};
use openwoot_protocol::Version;
use static_cell::StaticCell;

use crate::{
    Board, KEY_COLUMNS, KEY_ROWS, UsbDriver, hal,
    keys::{self, LINKED_PROFILES, PROFILES},
    lighting::{self, COLUMNS, LedDriver, PAGE_LENGTH, PAGES, ROWS, Task},
    scan, storage,
};

pub type Mutual = CriticalSectionRawMutex;
pub type Driver = Negotiated<UsbDriver>;
pub type ProfileConfigurator =
    Configurator<'static, Mutual, storage::Nor, Bar, KEY_ROWS, KEY_COLUMNS>;
type ProfileModel = Model<'static, Mutual, storage::Nor, Bar, KEY_ROWS, KEY_COLUMNS>;
type FirmwareState = State<Mutual, Bar, ROWS, COLUMNS, PAGES, PAGE_LENGTH>;

static STATE: StaticCell<FirmwareState> = StaticCell::new();
static MODEL: StaticCell<RefCell<ProfileModel>> = StaticCell::new();
static CHANNEL: Signal<Mutual, Channel<'static, Driver>> = Signal::new();

pub struct Tasks {
    pub keys: KeyTask<'static, Mutual>,
    pub usb: UsbParts<'static, UsbDriver, Mutual>,
    pub key_reports: &'static KeyReports<Mutual>,
    pub handler: Handler<ProfileConfigurator>,
    pub bootloader: hal::system::Bootloader,
    pub lighting: Task,
    pub leds: LedDriver,
    pub travels: Travels<'static, Mutual, { scan::INPUTS }, { scan::ADDRESSES }>,
    pub profile_keys: ProfileConfigurator,
}

#[inline(never)]
pub fn start(mut board: Board, version: Version) -> Tasks {
    let state: &'static FirmwareState = STATE.init(State::new());
    let identity = board.identity();
    let variant = board.storage.layout_variant;
    let model = open_model(&mut board, state);
    let handler = Handler::with_configuration(
        version,
        board.stock_identity(),
        Configurator::new(model, keys::capabilities(board.unit.edition))
            .with_geometry(&keys::GEOMETRY)
            .with_negotiated_speed(hal::usb::negotiated_speed),
    )
    .with_diagnostics(state.readout(hal::usb::negotiated_speed, hal::diagnostics::hardware))
    .with_host(state.host())
    .with_host_control(state.host_control())
    .with_sensors(state.sensors());
    let profile_keys = Configurator::new(model, keys::capabilities(board.unit.edition));
    let face = board.face;
    let firmware = Firmware::new(
        state,
        board.usb,
        hal::usb::negotiated_speed,
        face,
        identity,
        lighting::lighting(variant),
    );
    let travels = Travels {
        travel: scan::SCAN.travel(),
        map: keys::sensor_map(variant),
        shared: state.keys(),
        mode: face.mode,
    };
    Tasks {
        keys: firmware.keys,
        usb: firmware.usb,
        key_reports: firmware.key_reports,
        handler,
        bootloader: board.bootloader,
        lighting: firmware.lighting,
        leds: board.leds,
        travels,
        profile_keys,
    }
}

#[inline(never)]
fn open_model(board: &mut Board, state: &'static FirmwareState) -> &'static RefCell<ProfileModel> {
    if board.storage.opened.is_err() {
        state.diagnostics().storage_loaded(&board.storage.opened);
    }
    let model = MODEL.init(RefCell::new(
        Model::new(
            state.keys(),
            state.profile(),
            board
                .storage
                .settings
                .take()
                .map(|settings| settings.with_diagnostics(state.diagnostics())),
            board.unit.factory,
            board.storage.layout_variant,
            PROFILES,
        )
        .with_factory_variant(board.storage.factory_layout_variant)
        .with_linked(LINKED_PROFILES),
    ));
    model.get_mut().load();
    model
}

pub fn hand_over(channel: Channel<'static, Driver>) {
    CHANNEL.signal(channel);
}

#[embassy_executor::task]
pub async fn channel(
    handler: Handler<ProfileConfigurator>,
    mut bootloader: hal::system::Bootloader,
) {
    CHANNEL
        .wait()
        .await
        .run_timed(handler, &mut bootloader)
        .await;
}

#[embassy_executor::task]
pub async fn profiles(profile_keys: ProfileConfigurator) {
    profile_keys.serve().await;
}

#[embassy_executor::task]
pub async fn lights(
    mut task: Task,
    mut leds: LedDriver,
    travels: Travels<'static, Mutual, { scan::INPUTS }, { scan::ADDRESSES }>,
) {
    task.run(&mut leds, &travels).await;
}
