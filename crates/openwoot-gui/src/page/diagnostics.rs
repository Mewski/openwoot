use std::time::{Duration, Instant};

use gpui_kit::{
    ClipboardItem, Context, Div, Entity, Hsla, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Task, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        notification::Notification,
        switch::Switch,
        tab::{Tab, TabBar},
        v_flex,
    },
    div, hsla,
};
use openwoot_protocol::{
    device::Layout,
    diagnostics::{Bus, BusErrors, Lighting, Report, Usb},
    endpoints::{GetDiagnostics, GetSensorValues, SetKeyReports, SetRawScanning},
    profile::Slot,
    sensors::{self, Reading},
};

use super::Page;
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
    (Reading::Value, "Sensor"),
    (Reading::RestLevel, "Rest"),
    (Reading::BottomLevel, "Bottom"),
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
        let mut text = text_of(&device_lines(keyboard));
        if let Some(Ok(report)) = &self.report {
            for (group, lines) in report_groups(report, keyboard) {
                text.push_str("\n[");
                text.push_str(group);
                text.push_str("]\n");
                text.push_str(&text_of(&lines));
            }
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        window.push_notification(Notification::info("Copied"), cx);
    }

    fn reading_tabs(&self, cx: &Context<Self>) -> TabBar {
        let selected = READINGS
            .iter()
            .position(|(reading, _)| *reading == self.reading)
            .unwrap_or_default();
        TabBar::new("sensor-reading")
            .segmented()
            .selected_index(selected)
            .on_click(cx.listener(|this, index: &usize, _, cx| {
                if let Some((reading, _)) = READINGS.get(*index) {
                    this.choose_reading(*reading, cx);
                }
            }))
            .children(READINGS.map(|(_, label)| Tab::new().label(label)))
    }

    fn keyboard(&self, keyboard: &Keyboard, window: &Window) -> KeyboardView {
        let reading = self.reading;
        let values = self.values.clone().unwrap_or_default();
        let range = spread(&values);
        KeyboardView::new("sensor-readout", keyboard, |index| {
            let legend = usage::legend(keyboard.legends.get(index).copied().unwrap_or_default());
            let Some(value) = values.get(index).copied().flatten() else {
                return KeyFace {
                    legend: legend.into(),
                    dimmed: true,
                    ..KeyFace::default()
                };
            };
            if reading == Reading::Travel {
                let fraction = f32::from(value.min(MAX_TRAVEL)) / f32::from(MAX_TRAVEL);
                return KeyFace {
                    legend: legend.into(),
                    detail: Some(format!("{:.2}", fraction * keyboard.travel_mm(index)).into()),
                    fill: None,
                    travel: Some(fraction),
                    dimmed: false,
                };
            }
            KeyFace {
                legend: legend.into(),
                detail: Some(value.to_string().into()),
                fill: range.map(|(low, high)| heat(level(value, low, high))),
                travel: None,
                dimmed: false,
            }
        })
        .fit_width(panel::content_width(window))
    }

    fn caption(&self, cx: &Context<Self>) -> Option<Div> {
        let theme = cx.theme();
        let caption = h_flex()
            .w_full()
            .justify_center()
            .gap_2()
            .text_xs()
            .text_color(theme.muted_foreground);
        if let Some(reason) = &self.sensor_failure {
            return Some(caption.text_color(theme.danger).child(reason.clone()));
        }
        if self.reading == Reading::Travel {
            return None;
        }
        let (low, high) = spread(self.values.as_deref()?)?;
        let swatch = |color: Hsla| div().size_2p5().rounded_full().bg(color);
        Some(
            caption
                .child(swatch(heat(0.0)))
                .child(low.to_string())
                .child(swatch(heat(1.0)))
                .child(high.to_string()),
        )
    }

    fn device_section(keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        panel::section("Device", None, cx)
            .child(
                v_flex().gap_1().children(
                    device_lines(keyboard)
                        .into_iter()
                        .map(|line| value_line(line, cx)),
                ),
            )
            .child(
                h_flex().justify_end().child(
                    Button::new("copy-device")
                        .ghost()
                        .small()
                        .icon(Catalog::Clipboard)
                        .label("Copy")
                        .on_click(cx.listener(|this, _, window, cx| this.copy(window, cx))),
                ),
            )
    }

    fn testing_section(&self, cx: &Context<Self>) -> Div {
        let changed = self.raw_scanning || self.key_reports_off;
        panel::section("Testing", None, cx)
            .child(panel::row(
                "Raw scanning",
                Some("Shows raw converter values; keys do nothing".into()),
                Switch::new("raw-scanning")
                    .checked(self.raw_scanning)
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        this.set_raw_scanning(*checked, cx);
                    })),
                cx,
            ))
            .child(panel::row(
                "Key reports",
                Some("Turn off to test keys without typing".into()),
                Switch::new("key-reports")
                    .checked(!self.key_reports_off)
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        this.set_key_reports(*checked, cx);
                    })),
                cx,
            ))
            .children(
                changed.then(|| {
                    panel::note("Keys do not type. Both reset when you leave this page.", cx)
                }),
            )
    }

    fn status_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let section = panel::section("Status", None, cx);
        let report = match &self.report {
            None => return section,
            Some(Err(reason)) => {
                return section.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(reason.clone()),
                );
            }
            Some(Ok(report)) => report,
        };
        let groups: Vec<Div> = report_groups(report, keyboard)
            .into_iter()
            .map(|(group, lines)| {
                v_flex()
                    .gap_1()
                    .child(panel::note(group, cx))
                    .children(lines.into_iter().map(|line| value_line(line, cx)))
            })
            .collect();
        let half = groups.len().div_ceil(2);
        let mut groups = groups.into_iter();
        let left: Vec<Div> = groups.by_ref().take(half).collect();
        section.child(
            h_flex()
                .items_start()
                .gap_6()
                .child(v_flex().flex_1().min_w_0().gap_4().children(left))
                .child(v_flex().flex_1().min_w_0().gap_4().children(groups)),
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
        let page = panel::page();
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return page.child(panel::note("No keyboard", cx));
        };
        let left = v_flex()
            .gap_4()
            .child(Self::device_section(&keyboard, cx))
            .child(self.testing_section(cx));
        page.child(panel::stage(self.keyboard(&keyboard, window)))
            .children(self.caption(cx))
            .child(panel::heading(
                Page::Diagnostics.title(),
                Some("Live sensor readout and status of the keyboard".into()),
                self.reading_tabs(cx),
                cx,
            ))
            .child(panel::columns(left, self.status_section(&keyboard, cx)))
    }
}

fn value_line((label, value): Line, cx: &Context<DiagnosticsPage>) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .gap_4()
        .justify_between()
        .text_sm()
        .child(div().text_color(theme.foreground).child(label))
        .child(div().text_color(theme.muted_foreground).child(value))
}

fn device_lines(keyboard: &Keyboard) -> Vec<Line> {
    let capabilities = &keyboard.capabilities;
    vec![
        ("Keyboard", keyboard.name.to_string()),
        ("Firmware", keyboard.version.firmware.to_string()),
        ("Protocol", keyboard.version.protocol.to_string()),
        (
            "Key matrix",
            format!(
                "{} × {} ({} keys)",
                capabilities.key_rows,
                capabilities.key_columns,
                capabilities.key_count()
            ),
        ),
        (
            "Profiles",
            format!("{} of {}", keyboard.profiles.len(), capabilities.profiles),
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
            "Layout",
            keyboard.layout.map_or_else(
                || "Unknown".to_owned(),
                |layout| layout_name(layout).to_owned(),
            ),
        ),
        (
            "Light bar",
            match capabilities.lightbar_leds {
                0 => "None".to_owned(),
                1 => "1 LED".to_owned(),
                leds => format!("{leds} LEDs"),
            },
        ),
        (
            "Host signals",
            if capabilities.host_signals {
                "Yes"
            } else {
                "No"
            }
            .to_owned(),
        ),
        ("Switch models", capabilities.switch_models.to_string()),
        (
            "Edition",
            capabilities
                .edition
                .map_or_else(|| "None".to_owned(), |edition| edition.to_string()),
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
                ("Cycles", scan.cycles.to_string()),
                ("Recoveries", scan.recoveries.to_string()),
                (
                    "Overruns",
                    scan.overruns.map_or_else(
                        || "Not detected".to_owned(),
                        |overruns| overruns.to_string(),
                    ),
                ),
            ],
        ),
        (
            "Keys",
            vec![
                ("Passes", keys.passes.to_string()),
                ("Reports", keys.reports.to_string()),
            ],
        ),
        (
            "Profile",
            vec![
                (
                    "Active",
                    profile.active.map_or_else(
                        || "None applied".to_owned(),
                        |active| keyboard.profile_name(Slot::new(active)).to_string(),
                    ),
                ),
                ("Requests", profile.requests.to_string()),
                ("Switches", profile.switches.to_string()),
            ],
        ),
        ("Lighting", lighting_lines(lighting)),
        ("LED bus", bus(led_bus)),
        (
            "Storage",
            vec![
                ("Loads", storage.loads.to_string()),
                ("Stores", storage.stores.to_string()),
                ("Failures", storage.failures.to_string()),
            ],
        ),
        ("Flash bus", bus(flash_bus)),
        ("USB", usb_lines(*usb)),
        (
            "Stack",
            vec![("Peak", format!("{} of {} bytes", stack.peak, stack.size))],
        ),
    ]
}

fn lighting_lines(lighting: &Lighting) -> Vec<Line> {
    vec![
        (
            "LED driver",
            if lighting.initialized {
                "Ready"
            } else {
                "Not initialized"
            }
            .to_owned(),
        ),
        (
            "LEDs",
            if lighting.active { "On" } else { "Off" }.to_owned(),
        ),
        ("Frames", lighting.frames.to_string()),
        ("Page writes", lighting.page_writes.to_string()),
        ("Failed writes", lighting.failed_page_writes.to_string()),
    ]
}

fn usb_lines(usb: Usb) -> Vec<Line> {
    vec![
        (
            "State",
            match (usb.configured, usb.suspended) {
                (true, false) => "Configured",
                (true, true) => "Configured, suspended",
                (false, false) => "Not configured",
                (false, true) => "Not configured, suspended",
            }
            .to_owned(),
        ),
        (
            "Speed",
            usb.speed
                .map_or_else(|| "Unknown".to_owned(), |speed| speed.to_string()),
        ),
        ("Resets", usb.resets.to_string()),
    ]
}

fn bus(bus: &Bus) -> Vec<Line> {
    vec![
        ("Transfers", bus.completed.to_string()),
        ("Errors", error_counts(&bus.errors)),
        (
            "Last error",
            bus.last_error
                .map_or_else(|| "None".to_owned(), |error| error.to_string()),
        ),
    ]
}

fn error_counts(errors: &BusErrors) -> String {
    let total = errors.total();
    if total == 0 {
        return "None".to_owned();
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
    lines
        .iter()
        .map(|(label, value)| format!("{label}: {value}\n"))
        .collect::<Vec<_>>()
        .concat()
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
        assert_eq!(error_counts(&BusErrors::default()), "None");
        let lines = bus(&Bus {
            completed: 7,
            errors,
            last_error: Some(BusError::NoAcknowledge),
        });
        assert_eq!(
            lines.last().map(|(_, value)| value.as_str()),
            Some("no acknowledge")
        );
    }

    #[test]
    fn copied_text_has_one_line_per_entry() {
        let lines = vec![
            ("Keyboard", "Wooting 80HE".to_owned()),
            ("Edition", "None".to_owned()),
        ];
        assert_eq!(text_of(&lines), "Keyboard: Wooting 80HE\nEdition: None\n");
    }
}
