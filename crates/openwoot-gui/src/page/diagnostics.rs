use std::time::{Duration, Instant};

use gpui_kit::{
    ClipboardItem, Context, Div, Entity, FontWeight, Hsla, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Task, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        notification::Notification,
        radio::{Radio, RadioGroup},
        switch::Switch,
        v_flex,
    },
    div, hsla,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    device::Layout,
    diagnostics::{Bus, BusErrors, Lighting, Report, Usb},
    endpoints::{GetDiagnostics, GetSensorValues, SetKeyReports, SetRawScanning},
    profile::Slot,
    sensors::{self, Reading},
};

use crate::{
    device::Device,
    keyboard::{KeyFace, KeyboardView},
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    usage,
};

const POLL_INTERVAL: Duration = Duration::from_millis(33);
const REPORT_INTERVAL: Duration = Duration::from_secs(1);
const VISIBLE_WITHIN: Duration = Duration::from_millis(500);
const MAX_TRAVEL: u16 = 4095;
const COLD_HUE: f32 = 0.62;
const READINGS: [(Reading, &str); 4] = [
    (Reading::Travel, "Travel"),
    (Reading::Value, "Sensor value"),
    (Reading::RestLevel, "Rest level"),
    (Reading::BottomLevel, "Bottom level"),
];

type Line = (&'static str, String);

pub(crate) struct DiagnosticsPage {
    store: Entity<Store>,
    device: Device,
    report: Option<Result<Report, SharedString>>,
    reported: Option<Instant>,
    reading: Reading,
    values: Option<sensors::Values>,
    sensor_failure: Option<SharedString>,
    raw_scanning: bool,
    key_reports_off: bool,
    rendered: Instant,
    _poll: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl DiagnosticsPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, _, cx| match event {
                StoreEvent::Connected | StoreEvent::Disconnected => {
                    this.raw_scanning = false;
                    this.key_reports_off = false;
                    this.values = None;
                    this.report = None;
                    this.reported = None;
                    cx.notify();
                }
                StoreEvent::ProfileChanged | StoreEvent::Reloaded | StoreEvent::Failed(_) => {}
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
        ];
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let Ok(step) = this.update(cx, DiagnosticsPage::poll) else {
                    return;
                };
                if let Some(step) = step {
                    step.await;
                }
            }
        });
        let device = store.read(cx).device();
        Self {
            store,
            device,
            report: None,
            reported: None,
            reading: Reading::Travel,
            values: None,
            sensor_failure: None,
            raw_scanning: false,
            key_reports_off: false,
            rendered: Instant::now(),
            _poll: poll,
            _subscriptions: subscriptions,
        }
    }

    fn poll(&mut self, cx: &mut Context<Self>) -> Option<Task<()>> {
        self.store.read(cx).keyboard()?;
        if self.rendered.elapsed() > VISIBLE_WITHIN {
            self.restore_runtime_state();
            return None;
        }
        let reading = self.reading;
        let values = self
            .device
            .configure::<GetSensorValues, sensors::Values>(reading);
        let report_due = self
            .reported
            .is_none_or(|reported| reported.elapsed() >= REPORT_INTERVAL);
        let report = report_due.then(|| {
            self.reported = Some(Instant::now());
            self.device.call::<GetDiagnostics>(())
        });
        Some(cx.spawn(async move |this, cx| {
            let values = values.await;
            let report = match report {
                Some(report) => Some(report.await),
                None => None,
            };
            this.update(cx, |page, cx| {
                if page.reading == reading {
                    match values {
                        Ok(values) => {
                            page.values = Some(values);
                            page.sensor_failure = None;
                        }
                        Err(error) => page.sensor_failure = Some(error.to_string().into()),
                    }
                }
                if let Some(report) = report {
                    page.report = Some(report.map_err(|error| error.to_string().into()));
                }
                cx.notify();
            })
            .ok();
        }))
    }

    fn restore_runtime_state(&mut self) {
        if self.raw_scanning {
            self.raw_scanning = false;
            drop(self.device.call::<SetRawScanning>(false));
        }
        if self.key_reports_off {
            self.key_reports_off = false;
            drop(self.device.call::<SetKeyReports>(true));
        }
    }

    fn choose_reading(&mut self, reading: Reading, cx: &mut Context<Self>) {
        self.reading = reading;
        self.values = None;
        cx.notify();
    }

    fn set_raw_scanning(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let applied = self.store.update(cx, |store, cx| {
            store.apply::<SetRawScanning>(enabled, Scope::Runtime, cx)
        });
        cx.spawn(async move |this, cx| {
            if applied.await.is_ok() {
                this.update(cx, |page, cx| {
                    page.raw_scanning = enabled;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn set_key_reports(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let applied = self.store.update(cx, |store, cx| {
            store.apply::<SetKeyReports>(enabled, Scope::Runtime, cx)
        });
        cx.spawn(async move |this, cx| {
            if applied.await.is_ok() {
                this.update(cx, |page, cx| {
                    page.key_reports_off = !enabled;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn copy(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let mut lines = device_lines(keyboard);
        if let Some(Ok(report)) = &self.report {
            lines.extend(
                report_groups(report, keyboard)
                    .into_iter()
                    .flat_map(|(_, lines)| lines),
            );
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text_of(&lines)));
        window.push_notification(Notification::info("Copied the device information"), cx);
    }

    fn device_section(keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let mut section = panel::section(
            "Device",
            Some("What the keyboard reports about itself. Copy it into a bug report.".into()),
            cx,
        )
        .child(
            h_flex().justify_end().child(
                Button::new("copy-device")
                    .outline()
                    .icon(Icon::new(Catalog::Clipboard))
                    .label("Copy to clipboard")
                    .on_click(cx.listener(|this, _, window, cx| this.copy(window, cx))),
            ),
        );
        for (label, value) in device_lines(keyboard) {
            section = section.child(panel::row(label, None, div().text_sm().child(value), cx));
        }
        section
    }

    fn report_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let section = panel::section(
            "Status",
            Some("Counters and states of every subsystem, refreshed every second.".into()),
            cx,
        );
        match &self.report {
            None => section.child(panel::note("Reading the status…", cx)),
            Some(Err(reason)) => section.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(reason.clone()),
            ),
            Some(Ok(report)) => {
                let theme = cx.theme();
                section.child(
                    h_flex().flex_wrap().gap_3().items_start().children(
                        report_groups(report, keyboard)
                            .into_iter()
                            .map(|(title, lines)| {
                                v_flex()
                                    .w(px(300.0))
                                    .gap_1()
                                    .p_3()
                                    .rounded(theme.radius)
                                    .border_1()
                                    .border_color(theme.border)
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(title),
                                    )
                                    .children(lines.into_iter().map(|(label, value)| {
                                        h_flex()
                                            .justify_between()
                                            .gap_2()
                                            .text_xs()
                                            .child(
                                                div()
                                                    .text_color(theme.muted_foreground)
                                                    .child(label),
                                            )
                                            .child(div().text_right().child(value))
                                    }))
                            }),
                    ),
                )
            }
        }
    }

    fn sensor_section(&self, keyboard: &Keyboard, window: &Window, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let reading = self.reading;
        let values = self.values.clone().unwrap_or_default();
        let range = spread(&values);
        let board = KeyboardView::new("sensor-readout", keyboard, |index| {
            let legend = usage::legend(keyboard.legends.get(index).copied().unwrap_or_default());
            let Some(value) = values.get(index).copied().flatten() else {
                return KeyFace {
                    legend: legend.into(),
                    detail: Some("–".into()),
                    dimmed: true,
                    ..KeyFace::default()
                };
            };
            match reading {
                Reading::Travel => {
                    let fraction = f32::from(value.min(MAX_TRAVEL)) / f32::from(MAX_TRAVEL);
                    KeyFace {
                        legend: legend.into(),
                        detail: Some(format!("{:.2}", fraction * keyboard.travel_mm(index)).into()),
                        fill: Some(heat(fraction)),
                        travel: Some(fraction),
                        dimmed: false,
                    }
                }
                Reading::Value | Reading::RestLevel | Reading::BottomLevel => KeyFace {
                    legend: legend.into(),
                    detail: Some(value.to_string().into()),
                    fill: range.map(|(low, high)| heat(level(value, low, high))),
                    travel: None,
                    dimmed: false,
                },
            }
        })
        .fit_width(window.viewport_size().width - px(340.0));
        let selected = READINGS
            .iter()
            .position(|(candidate, _)| *candidate == reading);
        let caption = match reading {
            Reading::Travel => "Key travel in millimetres, from the calibrated rest and bottom levels.".to_owned(),
            Reading::Value => if self.raw_scanning {
                "The raw converter value of every sensor (raw scanning is on)."
            } else {
                "The filtered sensor value of every key."
            }
            .to_owned(),
            Reading::RestLevel => "The calibrated sensor level of every key at rest, in the current scan mode.".to_owned(),
            Reading::BottomLevel => "The calibrated sensor level of every key when pressed to the bottom, in the current scan mode.".to_owned(),
        };
        let range_note = match (reading, range) {
            (Reading::Travel, _) | (_, None) => None,
            (_, Some((low, high))) => Some(format!("Colours span {low} (blue) to {high} (red).")),
        };
        panel::section(
            "Sensors",
            Some("Live sensor readout of every key while this page is open.".into()),
            cx,
        )
        .child(
            RadioGroup::horizontal("reading")
                .children(
                    READINGS
                        .iter()
                        .map(|(_, label)| Radio::new(*label).label(*label)),
                )
                .selected_index(selected)
                .on_change(cx.listener(|this, index: &usize, _, cx| {
                    if let Some((reading, _)) = READINGS.get(*index) {
                        this.choose_reading(*reading, cx);
                    }
                })),
        )
        .child(panel::note(caption, cx))
        .children(range_note.map(|note| panel::note(note, cx)))
        .children(
            self.sensor_failure
                .clone()
                .map(|reason| div().text_sm().text_color(theme.danger).child(reason)),
        )
        .child(div().w_full().overflow_hidden().child(board))
    }

    fn controls_section(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        panel::section(
            "Troubleshooting",
            Some("Both switches return to normal when you leave this page or the keyboard reconnects.".into()),
            cx,
        )
        .child(panel::row(
            "Raw scanning",
            Some(
                "Keep the raw converter value of every sensor and evaluate no key, to check a \
                 sensor without the calibration in the way. Keys do nothing while it is on."
                    .into(),
            ),
            Switch::new("raw-scanning")
                .checked(self.raw_scanning)
                .on_change(cx.listener(|this, checked: &bool, _, cx| this.set_raw_scanning(*checked, cx))),
            cx,
        ))
        .child(panel::row(
            "Key reports",
            Some(
                "Turn off to test keys without typing: the keyboard sends no key presses, Fn \
                 actions do not run and advanced keys rest; the gamepad still follows the keys."
                    .into(),
            ),
            Switch::new("key-reports")
                .checked(!self.key_reports_off)
                .on_change(cx.listener(|this, checked: &bool, _, cx| this.set_key_reports(*checked, cx))),
            cx,
        ))
        .when(self.key_reports_off, |this| {
            this.child(
                h_flex()
                    .gap_2()
                    .text_sm()
                    .text_color(theme.warning)
                    .child(Icon::new(Catalog::Keyboard))
                    .child("Key reports are off: the keyboard does not type."),
            )
        })
        .when(self.raw_scanning, |this| {
            this.child(
                h_flex()
                    .gap_2()
                    .text_sm()
                    .text_color(theme.warning)
                    .child(Icon::new(Catalog::ScanLine))
                    .child("Raw scanning is on: no key works until it is turned off."),
            )
        })
        .child(
            h_flex().child(
                Button::new("restore")
                    .ghost()
                    .label("Return both to normal")
                    .disabled(!self.raw_scanning && !self.key_reports_off)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.raw_scanning {
                            this.set_raw_scanning(false, cx);
                        }
                        if this.key_reports_off {
                            this.set_key_reports(true, cx);
                        }
                    })),
            ),
        )
    }
}

impl Drop for DiagnosticsPage {
    fn drop(&mut self) {
        self.restore_runtime_state();
    }
}

impl Render for DiagnosticsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered = Instant::now();
        let page = panel::page(
            "Diagnostics",
            "Device information, live status counters and the sensor readout, for troubleshooting.",
            cx,
        );
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return page.child(panel::note(
                "Connect a keyboard to see its diagnostics.",
                cx,
            ));
        };
        page.child(
            v_flex()
                .gap_4()
                .max_w_full()
                .child(Self::device_section(&keyboard, cx))
                .child(self.controls_section(cx))
                .child(self.sensor_section(&keyboard, window, cx))
                .child(self.report_section(&keyboard, cx)),
        )
    }
}

fn device_lines(keyboard: &Keyboard) -> Vec<Line> {
    let capabilities = &keyboard.capabilities;
    vec![
        ("Keyboard", keyboard.name.to_string()),
        (
            "Firmware",
            format!("openwoot {}", keyboard.version.firmware),
        ),
        ("Protocol", format!("version {}", keyboard.version.protocol)),
        (
            "Key matrix",
            format!(
                "{} rows × {} columns ({} keys)",
                capabilities.key_rows,
                capabilities.key_columns,
                capabilities.key_count()
            ),
        ),
        (
            "Profiles",
            format!(
                "{} of {} slots",
                keyboard.profiles.len(),
                capabilities.profiles
            ),
        ),
        (
            "Layouts",
            capabilities
                .layouts
                .iter()
                .map(|layout| layout_name(*layout))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        (
            "Layout in effect",
            keyboard.layout.map_or_else(
                || "not reported".to_owned(),
                |layout| layout_name(layout).to_owned(),
            ),
        ),
        (
            "Light bar",
            match capabilities.lightbar_leds {
                0 => "none".to_owned(),
                1 => "1 LED".to_owned(),
                leds => format!("{leds} LEDs"),
            },
        ),
        (
            "Host signals",
            if capabilities.host_signals {
                "supported"
            } else {
                "not supported"
            }
            .to_owned(),
        ),
        ("Switch models", capabilities.switch_models.to_string()),
        (
            "Edition",
            capabilities
                .edition
                .map_or_else(|| "none".to_owned(), |edition| edition.to_string()),
        ),
    ]
}

fn report_groups(report: &Report, keyboard: &Keyboard) -> Vec<(&'static str, Vec<Line>)> {
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
    } = report;
    vec![
        (
            "Scan",
            vec![
                ("Scan cycles", scan.cycles.to_string()),
                ("Scan recoveries", scan.recoveries.to_string()),
                (
                    "Scan overruns",
                    scan.overruns.map_or_else(
                        || "not detected by this chip".to_owned(),
                        |overruns| overruns.to_string(),
                    ),
                ),
            ],
        ),
        (
            "Keys",
            vec![
                ("Key passes", keys.passes.to_string()),
                ("Key reports", keys.reports.to_string()),
            ],
        ),
        (
            "Profile",
            vec![
                (
                    "Active profile",
                    profile.active.map_or_else(
                        || "none applied".to_owned(),
                        |active| keyboard.profile_name(Slot::new(active)).to_string(),
                    ),
                ),
                ("Profile requests", profile.requests.to_string()),
                ("Profile switches", profile.switches.to_string()),
            ],
        ),
        ("Lighting", lighting_lines(lighting)),
        (
            "LED bus",
            bus(
                ["LED bus transfers", "LED bus errors", "LED bus last error"],
                led_bus,
            ),
        ),
        (
            "Storage",
            vec![
                ("Storage loads", storage.loads.to_string()),
                ("Storage stores", storage.stores.to_string()),
                ("Storage failures", storage.failures.to_string()),
            ],
        ),
        (
            "Flash bus",
            bus(
                [
                    "Flash bus transfers",
                    "Flash bus errors",
                    "Flash bus last error",
                ],
                flash_bus,
            ),
        ),
        ("USB", usb_lines(*usb)),
        (
            "Stack",
            vec![(
                "Stack peak",
                format!("{} of {} bytes", stack.peak, stack.size),
            )],
        ),
    ]
}

fn lighting_lines(lighting: &Lighting) -> Vec<Line> {
    vec![
        (
            "LED driver",
            if lighting.initialized {
                "initialized"
            } else {
                "not initialized"
            }
            .to_owned(),
        ),
        (
            "Lighting",
            if lighting.active { "on" } else { "off" }.to_owned(),
        ),
        ("Lighting frames", lighting.frames.to_string()),
        ("LED page writes", lighting.page_writes.to_string()),
        (
            "Failed page writes",
            lighting.failed_page_writes.to_string(),
        ),
    ]
}

fn usb_lines(usb: Usb) -> Vec<Line> {
    vec![
        (
            "USB state",
            match (usb.configured, usb.suspended) {
                (true, false) => "configured",
                (true, true) => "configured, suspended",
                (false, false) => "not configured",
                (false, true) => "not configured, suspended",
            }
            .to_owned(),
        ),
        (
            "USB speed",
            usb.speed
                .map_or_else(|| "unknown".to_owned(), |speed| speed.to_string()),
        ),
        ("USB resets", usb.resets.to_string()),
    ]
}

fn bus([transfers, errors, last_error]: [&'static str; 3], bus: &Bus) -> Vec<Line> {
    vec![
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

fn text_of(lines: &[Line]) -> String {
    let mut text: String = lines
        .iter()
        .map(|(label, value)| format!("{label}: {value}"))
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    text
}

const fn layout_name(layout: Layout) -> &'static str {
    match layout {
        Layout::Ansi => "ANSI",
        Layout::Iso => "ISO",
        Layout::Jis => "JIS",
    }
}

fn spread(values: &[Option<u16>]) -> Option<(u16, u16)> {
    let present = values.iter().flatten().copied();
    let low = present.clone().min()?;
    let high = present.max()?;
    Some((low, high))
}

fn level(value: u16, low: u16, high: u16) -> f32 {
    if high <= low {
        return 0.0;
    }
    f32::from(value.saturating_sub(low)) / f32::from(high - low)
}

fn heat(level: f32) -> Hsla {
    hsla(COLD_HUE * (1.0 - level.clamp(0.0, 1.0)), 0.75, 0.45, 1.0)
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::diagnostics::BusError;

    use super::*;

    #[test]
    fn spread_ignores_keys_without_a_sensor() {
        assert_eq!(
            spread(&[None, Some(900), Some(1200), None, Some(1000)]),
            Some((900, 1200))
        );
        assert_eq!(spread(&[None, None]), None);
    }

    #[test]
    fn level_places_a_value_within_the_spread() {
        assert!((level(1050, 900, 1200) - 0.5).abs() < f32::EPSILON);
        assert!(level(900, 900, 900).abs() < f32::EPSILON);
    }

    #[test]
    fn bus_errors_list_every_kind_that_occurred() {
        let errors = BusErrors {
            timeout: 2,
            no_acknowledge: 1,
            ..BusErrors::default()
        };
        assert_eq!(error_counts(&errors), "3 (2 timeout, 1 no acknowledge)");
        assert_eq!(error_counts(&BusErrors::default()), "none");
        let lines = bus(
            ["transfers", "errors", "last error"],
            &Bus {
                completed: 7,
                errors,
                last_error: Some(BusError::NoAcknowledge),
            },
        );
        assert_eq!(
            lines.last().map(|(_, value)| value.as_str()),
            Some("no acknowledge")
        );
    }

    #[test]
    fn copied_text_has_one_line_per_entry() {
        let lines = vec![
            ("Keyboard", "Wooting 80HE".to_owned()),
            ("Edition", "none".to_owned()),
        ];
        assert_eq!(text_of(&lines), "Keyboard: Wooting 80HE\nEdition: none\n");
    }
}
