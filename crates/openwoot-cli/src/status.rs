use std::fmt;

use openwoot_protocol::{
    configuration::Rejection,
    diagnostics::{Bus, BusErrors, Report, StorageUsage},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Status {
    pub(crate) device: String,
    pub(crate) report: Report,
    pub(crate) storage_usage: Result<StorageUsage, Rejection>,
}

type Line = (&'static str, String);

impl fmt::Display for Status {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lines = self.lines();
        let width = lines
            .iter()
            .map(|(label, _)| label.len() + 1)
            .max()
            .unwrap_or_default();
        for (label, value) in lines {
            writeln!(formatter, "{:<width$} {value}", format!("{label}:"))?;
        }
        Ok(())
    }
}

impl Status {
    fn lines(&self) -> Vec<Line> {
        let Report {
            scan,
            keys,
            profile,
            lighting,
            led_bus,
            storage,
            flash_bus,
            usb,
            stack,
        } = &self.report;
        let mut lines = vec![
            ("device", self.device.clone()),
            ("scan cycles", scan.cycles.to_string()),
            ("scan recoveries", scan.recoveries.to_string()),
            (
                "scan overruns",
                scan.overruns.map_or_else(
                    || "not detected by this chip".to_owned(),
                    |overruns| overruns.to_string(),
                ),
            ),
            ("key passes", keys.passes.to_string()),
            ("key reports", keys.reports.to_string()),
            (
                "active profile",
                profile
                    .active
                    .map_or_else(|| "none applied".to_owned(), |active| active.to_string()),
            ),
            ("profile requests", profile.requests.to_string()),
            ("profile switches", profile.switches.to_string()),
            (
                "led driver",
                if lighting.initialized {
                    "initialized"
                } else {
                    "not initialized"
                }
                .to_owned(),
            ),
            (
                "lighting",
                if lighting.active { "on" } else { "off" }.to_owned(),
            ),
            ("lighting frames", lighting.frames.to_string()),
            ("led page writes", lighting.page_writes.to_string()),
            (
                "failed page writes",
                lighting.failed_page_writes.to_string(),
            ),
        ];
        lines.extend(bus(
            ["led bus transfers", "led bus errors", "led bus last error"],
            led_bus,
        ));
        lines.extend([
            ("storage loads", storage.loads.to_string()),
            ("storage stores", storage.stores.to_string()),
            ("storage failures", storage.failures.to_string()),
            ("storage usage", storage_usage(self.storage_usage)),
        ]);
        lines.extend(bus(
            [
                "flash bus transfers",
                "flash bus errors",
                "flash bus last error",
            ],
            flash_bus,
        ));
        lines.extend([
            (
                "usb state",
                match (usb.configured, usb.suspended) {
                    (true, false) => "configured",
                    (true, true) => "configured, suspended",
                    (false, false) => "not configured",
                    (false, true) => "not configured, suspended",
                }
                .to_owned(),
            ),
            (
                "usb speed",
                usb.speed
                    .map_or_else(|| "unknown".to_owned(), |speed| speed.to_string()),
            ),
            ("usb resets", usb.resets.to_string()),
            (
                "stack peak",
                format!("{} of {} bytes", stack.peak, stack.size),
            ),
        ]);
        lines
    }
}

fn bus([transfers, errors, last_error]: [&'static str; 3], bus: &Bus) -> [Line; 3] {
    [
        (transfers, bus.completed.to_string()),
        (errors, error_counts(&bus.errors)),
        (
            last_error,
            bus.last_error
                .map_or_else(|| "none".to_owned(), |error| error.to_string()),
        ),
    ]
}

fn error_counts(errors: &BusErrors) -> String {
    let total = errors.total();
    if total == 0 {
        return "none".to_owned();
    }
    let kinds: Vec<String> = BusErrors::KINDS
        .iter()
        .filter_map(|&kind| {
            let count = errors.count(kind);
            (count > 0).then(|| format!("{count} {kind}"))
        })
        .collect();
    format!("{total} ({})", kinds.join(", "))
}

fn storage_usage(usage: Result<StorageUsage, Rejection>) -> String {
    match usage {
        Ok(usage) => format!(
            "{} of {} bytes in {}, {}",
            usage.used,
            usage.capacity,
            counted(usage.items, "item"),
            counted(usage.records, "current record"),
        ),
        Err(Rejection::Unsupported) => "no settings store".to_owned(),
        Err(rejection) => rejection.to_string(),
    }
}

fn counted(count: u32, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::diagnostics::{
        BusError, Keys, Lighting, Profile, Scan, Speed, Stack, Storage, Usb,
    };

    use super::*;

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
            storage_usage: Ok(StorageUsage {
                capacity: 524_288,
                used: 30_516,
                records: 8,
                items: 2_447,
            }),
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
                "storage usage:        30516 of 524288 bytes in 2447 items, 8 current records\n",
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
            storage_usage: Err(Rejection::Unsupported),
        }
        .to_string();
        for line in [
            "scan overruns:        not detected by this chip\n",
            "active profile:       none applied\n",
            "usb state:            not configured\n",
            "usb speed:            unknown\n",
            "storage usage:        no settings store\n",
        ] {
            assert!(status.contains(line), "{line:?} missing from\n{status}");
        }
    }

    #[test]
    fn one_item_reads_in_the_singular_and_a_failed_read_names_its_rejection() {
        let usage = StorageUsage {
            capacity: 16_384,
            used: 12,
            records: 1,
            items: 1,
        };
        assert_eq!(
            storage_usage(Ok(usage)),
            "12 of 16384 bytes in 1 item, 1 current record"
        );
        assert_eq!(
            storage_usage(Err(Rejection::StorageInaccessible)),
            "could not access the settings storage"
        );
    }
}
