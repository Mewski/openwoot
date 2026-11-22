use openwoot_cli::status::Status;
use openwoot_protocol::diagnostics::{
    Bus, BusError, BusErrors, Keys, Lighting, Profile, Report, Scan, Speed, Stack, Storage, Usb,
};

const DEVICE: &str = "31e3:1400 \"Wooting 80HE\"";

fn running() -> Report {
    Report {
        scan: Scan {
            cycles: 120_000,
            recoveries: 1,
            overruns: Some(0),
        },
        keys: Keys {
            passes: 119_990,
            reports: 42,
        },
        profile: Profile {
            active: Some(0),
            requests: 3,
            switches: 2,
        },
        lighting: Lighting {
            initialized: false,
            active: true,
            frames: 5000,
            page_writes: 0,
            failed_page_writes: 12,
        },
        led_bus: Bus {
            completed: 0,
            errors: BusErrors {
                timeout: 2,
                no_acknowledge: 10,
                ..BusErrors::default()
            },
            last_error: Some(BusError::NoAcknowledge),
        },
        storage: Storage {
            loads: 30,
            stores: 4,
            failures: 0,
        },
        flash_bus: Bus {
            completed: 812,
            ..Bus::default()
        },
        usb: Usb {
            configured: true,
            suspended: false,
            speed: Some(Speed::High),
            resets: 2,
        },
        stack: Stack {
            size: 125_904,
            peak: 21_480,
        },
    }
}

#[test]
fn status_prints_every_counter_as_one_aligned_label_and_value_per_line() {
    let status = Status {
        device: DEVICE.to_owned(),
        report: running(),
    };
    assert_eq!(
        status.to_string(),
        concat!(
            "device:               31e3:1400 \"Wooting 80HE\"\n",
            "scan cycles:          120000\n",
            "scan recoveries:      1\n",
            "scan overruns:        0\n",
            "key passes:           119990\n",
            "key reports:          42\n",
            "active profile:       0\n",
            "profile requests:     3\n",
            "profile switches:     2\n",
            "led driver:           not initialized\n",
            "lighting:             on\n",
            "lighting frames:      5000\n",
            "led page writes:      0\n",
            "failed page writes:   12\n",
            "led bus transfers:    0\n",
            "led bus errors:       12 (2 timeout, 10 no acknowledge)\n",
            "led bus last error:   no acknowledge\n",
            "storage loads:        30\n",
            "storage stores:       4\n",
            "storage failures:     0\n",
            "flash bus transfers:  812\n",
            "flash bus errors:     none\n",
            "flash bus last error: none\n",
            "usb state:            configured\n",
            "usb speed:            high speed (480 Mbit/s)\n",
            "usb resets:           2\n",
            "stack peak:           21480 of 125904 bytes\n",
        )
    );
}

#[test]
fn values_a_keyboard_has_not_reached_yet_are_named_rather_than_zero() {
    let status = Status {
        device: DEVICE.to_owned(),
        report: Report::default(),
    }
    .to_string();
    for line in [
        "scan overruns:        not detected by this chip\n",
        "active profile:       none applied\n",
        "usb state:            not configured\n",
        "usb speed:            unknown\n",
    ] {
        assert!(status.contains(line), "{line:?} missing from\n{status}");
    }
}
