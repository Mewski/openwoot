use std::fmt;

use openwoot_protocol::diagnostics::{Bus, BusErrors, Report};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub device: String,
    pub report: Report,
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
