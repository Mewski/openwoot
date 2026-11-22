use embassy_futures::block_on;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_usb::driver::Speed;
use openwoot::{diagnostics::Hardware, protocol::Handler, runtime::State};
use openwoot_protocol::{
    Error, FirmwareVersion, PROTOCOL_VERSION, Version, WireError, decode_response,
    diagnostics::{
        self, Bus, BusError, BusErrors, Keys, Lighting, Profile, Report, Scan, Stack, Usb,
    },
    encode_request,
    endpoints::GetDiagnostics,
    report,
};

use crate::support::stock_identity;

static STATE: State<CriticalSectionRawMutex, (), 1, 1, 1, 1> = State::new();

const SEQUENCE: u16 = 0x0D1A;
const VERSION: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 0,
        minor: 1,
        patch: 0,
    },
};
const LED_BUS: Bus = Bus {
    completed: 40,
    errors: BusErrors {
        timeout: 0,
        bus: 0,
        arbitration_loss: 0,
        no_acknowledge: 2,
        overrun: 0,
        dma: 0,
        other: 0,
    },
    last_error: Some(BusError::NoAcknowledge),
};
const STACK: Stack = Stack {
    size: 32_768,
    peak: 9_216,
};

fn hardware() -> Hardware {
    Hardware {
        scan_overruns: Some(3),
        led_bus: LED_BUS,
        flash_bus: Bus::default(),
        stack: STACK,
    }
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "the readout takes the chip's negotiated-speed accessor"
)]
const fn high_speed() -> Option<Speed> {
    Some(Speed::High)
}

fn ask(handler: &mut Handler) -> Result<Option<Report>, Error> {
    let mut request = [0; report::MAX_LEN];
    let request = encode_request::<GetDiagnostics>(SEQUENCE, &(), &mut request)?;
    let mut response = [0; report::MAX_LEN];
    let answer = block_on(handler.respond(request, &mut response));
    let Some(reply) = answer.reply else {
        return Ok(None);
    };
    decode_response::<GetDiagnostics>(SEQUENCE, reply)
}

#[test]
fn the_diagnostics_endpoint_reports_the_counters_of_every_subsystem() {
    let diagnostics = STATE.diagnostics();
    diagnostics.scan_cycle_completed();
    diagnostics.scan_cycle_completed();
    diagnostics.scan_recovered();
    diagnostics.keys_passed();
    diagnostics.key_report_published();
    diagnostics.profile_applied(0);
    diagnostics.profile_requested();
    diagnostics.profile_applied(1);
    diagnostics.set_lighting_initialized(true);
    diagnostics.set_lighting_active(true);
    diagnostics.frame_rendered();
    diagnostics.page_written::<(), ()>(&Ok(()));
    diagnostics.page_written::<(), ()>(&Err(()));
    diagnostics.storage_loaded::<(), ()>(&Ok(()));
    diagnostics.storage_stored::<(), ()>(&Err(()));
    let mut handler = Handler::new(VERSION, stock_identity(None))
        .with_diagnostics(STATE.readout(high_speed, hardware));
    assert_eq!(
        ask(&mut handler),
        Ok(Some(Report {
            scan: Scan {
                cycles: 2,
                recoveries: 1,
                overruns: Some(3),
            },
            keys: Keys {
                passes: 1,
                reports: 1,
            },
            profile: Profile {
                active: Some(1),
                requests: 1,
                switches: 1,
            },
            lighting: Lighting {
                initialized: true,
                active: true,
                frames: 1,
                page_writes: 1,
                failed_page_writes: 1,
            },
            led_bus: LED_BUS,
            storage: diagnostics::Storage {
                loads: 1,
                stores: 0,
                failures: 1,
            },
            flash_bus: Bus::default(),
            usb: Usb {
                configured: false,
                suspended: false,
                speed: Some(diagnostics::Speed::High),
                resets: 0,
            },
            stack: STACK,
        }))
    );
}

#[test]
fn a_keyboard_without_diagnostics_answers_the_request_as_unknown() {
    let mut handler = Handler::new(VERSION, stock_identity(None));
    assert_eq!(ask(&mut handler), Err(Error::Device(WireError::UnknownKey)));
}
