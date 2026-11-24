use std::{
    error::Error,
    path::{Path, PathBuf},
};

use gpui_kit::{
    Context, Div, Entity, IntoElement, ParentElement as _, PathPromptOptions, Render, SharedString,
    Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariant, ButtonVariants as _},
        h_flex,
        progress::Progress,
        v_flex,
    },
    div,
};
use openwoot_host::{
    flash::{self, Event, Plan},
    hid,
};

use super::Page;
use crate::{
    panel,
    store::{Link, Store},
};

pub(crate) struct FirmwarePage {
    store: Entity<Store>,
    image: Option<Image>,
    installation: Option<Installation>,
    _subscriptions: Vec<Subscription>,
}

enum Image {
    Checking(PathBuf),
    Planned(Box<Plan>),
    Rejected(SharedString),
}

#[derive(Default)]
struct Installation {
    steps: Vec<SharedString>,
    blocks: Option<Blocks>,
    outcome: Option<Result<(), SharedString>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Blocks {
    total: usize,
    sent: usize,
}

impl FirmwarePage {
    pub(crate) fn new(store: Entity<Store>, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let observation = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            store,
            image: None,
            installation: None,
            _subscriptions: vec![observation],
        }
    }

    fn installing(&self) -> bool {
        self.installation
            .as_ref()
            .is_some_and(|installation| installation.outcome.is_none())
    }

    fn choose_image(cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose firmware image".into()),
        });
        cx.spawn(async move |this, cx| {
            let path = match chosen.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Err(error)) => {
                    this.update(cx, |page, cx| {
                        page.image = Some(Image::Rejected(
                            format!("could not open the file dialog: {error:#}").into(),
                        ));
                        cx.notify();
                    })
                    .ok();
                    None
                }
                Ok(Ok(None)) | Err(_) => None,
            };
            let Some(path) = path else {
                return;
            };
            this.update(cx, |page, cx| {
                page.image = Some(Image::Checking(path.clone()));
                page.installation = None;
                cx.notify();
            })
            .ok();
            let planned = cx
                .background_executor()
                .spawn(async move { flash::plan(&path) })
                .await;
            this.update(cx, |page, cx| {
                page.image = Some(match planned {
                    Ok(plan) => Image::Planned(Box::new(plan)),
                    Err(error) => Image::Rejected(error_chain(&error).into()),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn confirm_install(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Image::Planned(plan)) = &self.image else {
            return;
        };
        let plan = plan.as_ref().clone();
        let page = cx.entity();
        let title = format!("Install firmware for “{}”?", plan.board);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            let plan = plan.clone();
            alert
                .title(title.clone())
                .description(
                    "The keyboard restarts and cannot type until the update finishes; keep it \
                     plugged in. Wootility restores the stock firmware.",
                )
                .confirm()
                .ok_text("Install")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    let plan = plan.clone();
                    page.update(cx, |page, cx| page.install(plan, cx));
                    true
                })
        });
    }

    fn install(&mut self, plan: Plan, cx: &mut Context<Self>) {
        if self.installing() {
            return;
        }
        let (sender, events) = async_channel::unbounded();
        let installed = self.store.read(cx).device().install(plan, sender);
        self.installation = Some(Installation::default());
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this
                    .update(cx, |page, cx| {
                        if let Some(installation) = &mut page.installation {
                            installation.record(&event);
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let outcome = installed
                .await
                .map_err(|error| SharedString::from(error_chain(&error)));
            this.update(cx, |page, cx| {
                if let Some(installation) = &mut page.installation {
                    installation.outcome = Some(outcome);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn installed_section(&self, cx: &Context<Self>) -> Div {
        let store = self.store.read(cx);
        let Some(keyboard) = store.keyboard() else {
            let reason = match store.link() {
                Link::Unreachable(reason) => Some(reason.clone()),
                Link::Searching | Link::Loading | Link::Connected => None,
            };
            return panel::section(
                "Installed",
                Some("No openwoot keyboard; update mode and stock firmware still work.".into()),
                cx,
            )
            .children(reason.map(|reason| panel::note(reason, cx)));
        };
        let section = panel::section("Installed", None, cx);
        section
            .child(value_row("Keyboard", keyboard.name.clone(), cx))
            .child(value_row(
                "Firmware",
                keyboard.version.firmware.to_string().into(),
                cx,
            ))
            .child(value_row(
                "Protocol",
                keyboard.version.protocol.to_string().into(),
                cx,
            ))
    }

    fn update_section(&self, cx: &mut Context<Self>) -> Div {
        let mut section = panel::section(
            "Update",
            Some("Choose an openwoot image (.hex or .bin) to check and install.".into()),
            cx,
        );
        match &self.image {
            None => {}
            Some(Image::Checking(path)) => {
                section = section.child(value_row("File", file_name(path), cx));
            }
            Some(Image::Rejected(reason)) => {
                section = section.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(reason.clone()),
                );
            }
            Some(Image::Planned(plan)) => {
                section = section.child(value_row("File", file_name(&plan.path), cx));
                for (label, value) in plan_lines(plan) {
                    section = section.child(value_row(label, value.into(), cx));
                }
                if plan.update.is_none() {
                    section = section.child(panel::note("This board has no update mode.", cx));
                } else if self.installation.is_none() {
                    section = section.child(
                        h_flex().justify_end().child(
                            Button::new("install")
                                .primary()
                                .small()
                                .label("Install…")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_install(window, cx);
                                })),
                        ),
                    );
                }
            }
        }
        if let Some(installation) = &self.installation {
            section = section.child(Self::progress(installation, cx));
        }
        section
    }

    fn progress(installation: &Installation, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let percent = installation.blocks.map_or(0.0, Blocks::percent);
        let outcome = installation.outcome.as_ref().map(|outcome| match outcome {
            Ok(()) => div().text_sm().text_color(theme.success).child("Installed"),
            Err(reason) => div()
                .text_sm()
                .text_color(theme.danger)
                .child(reason.clone()),
        });
        let running = installation.outcome.is_none();
        v_flex()
            .gap_2()
            .pt_2()
            .border_t_1()
            .border_color(theme.border)
            .child(
                Progress::new("upload-progress")
                    .loading(running && installation.blocks.is_none())
                    .value(if installation.outcome == Some(Ok(())) {
                        100.0
                    } else {
                        percent
                    }),
            )
            .child(
                v_flex().gap_0p5().children(
                    installation
                        .steps
                        .iter()
                        .map(|step| panel::note(step.clone(), cx)),
                ),
            )
            .children(running.then(|| panel::note("Keep the keyboard plugged in.", cx)))
            .children(outcome)
    }
}

fn value_row(label: &'static str, value: SharedString, cx: &Context<FirmwarePage>) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .gap_4()
        .justify_between()
        .text_sm()
        .child(div().text_color(theme.foreground).child(label))
        .child(div().text_color(theme.muted_foreground).child(value))
}

fn file_name(path: &Path) -> SharedString {
    path.file_name()
        .map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
        .into()
}

impl Installation {
    fn record(&mut self, event: &Event) {
        match event {
            Event::Writing { blocks } => {
                self.blocks = Some(Blocks {
                    total: *blocks,
                    sent: 0,
                });
            }
            Event::Sent { blocks } => {
                if let Some(progress) = &mut self.blocks {
                    progress.sent = *blocks;
                }
                return;
            }
            Event::FirmwareIdentified { .. }
            | Event::EnteringUpdateMode { .. }
            | Event::UpdateModeReached(_)
            | Event::BootloaderReady(_)
            | Event::CrcVerified(_)
            | Event::RestartFailed(_)
            | Event::StartingFirmware { .. }
            | Event::FirmwareStarted(_) => {}
        }
        self.steps.push(step_text(event).into());
    }
}

impl Blocks {
    fn percent(self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        let sent = u32::try_from(self.sent).unwrap_or(u32::MAX);
        let total = u32::try_from(self.total).unwrap_or(u32::MAX);
        #[expect(
            clippy::cast_precision_loss,
            reason = "block counts stay far below the 24-bit mantissa"
        )]
        let ratio = sent as f32 / total as f32;
        (ratio * 100.0).clamp(0.0, 100.0)
    }
}

impl Render for FirmwarePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let installed = self.installed_section(cx);
        let update = self.update_section(cx);
        let choose = Button::new("choose-image")
            .outline()
            .small()
            .icon(Catalog::FileUp)
            .label("Choose image…")
            .disabled(self.installing() || matches!(self.image, Some(Image::Checking(_))))
            .on_click(cx.listener(|_, _, _, cx| Self::choose_image(cx)));
        panel::page()
            .child(panel::heading(Page::Firmware.title(), None, choose, cx))
            .child(panel::columns(installed, update))
    }
}

fn plan_lines(plan: &Plan) -> Vec<(&'static str, String)> {
    let mut lines = vec![
        ("Board", plan.board.clone()),
        ("Size", format!("{} bytes", plan.image_len)),
        ("Address", hex(plan.area.start)),
        (
            "USB ids",
            format!(
                "{:04x}:{:04x}",
                plan.usb.vendor_id, plan.usb.product_id_base
            ),
        ),
    ];
    if let Some(update) = &plan.update {
        lines.push((
            "Update mode",
            format!("{:04x}:{:04x}", plan.usb.vendor_id, update.mode.product_id),
        ));
        lines.push((
            "Upload",
            format!(
                "{} of {} bytes",
                block_count(update.upload.block_count()),
                update.upload.block_size
            ),
        ));
        lines.push(("CRC-24", hex(update.upload.crc)));
    }
    lines
}

fn step_text(event: &Event) -> String {
    match event {
        Event::FirmwareIdentified { device, firmware } => {
            format!("Found {}, {firmware}", hid::describe(device))
        }
        Event::EnteringUpdateMode { timeout } => format!(
            "Asking the firmware to restart into update mode, waiting up to {} s",
            timeout.as_secs()
        ),
        Event::UpdateModeReached(device) => {
            format!("Update mode reached: {}", hid::describe(device))
        }
        Event::BootloaderReady(state) => format!(
            "Bootloader ready: {}, serial {}, flags {}, session CRC {}",
            state.version.as_ref().map_or_else(
                || "version not reported".to_owned(),
                |version| format!("version {version}")
            ),
            state.serial.as_deref().unwrap_or("not reported"),
            state
                .flags
                .map_or_else(|| "not reported".to_owned(), |flags| flags.to_string()),
            state
                .session_crc
                .map_or_else(|| "not reported".to_owned(), hex),
        ),
        Event::Writing { blocks } => format!(
            "Writing {}; until the update completes the keyboard starts in update mode",
            block_count(*blocks)
        ),
        Event::Sent { blocks } => format!("{} written", block_count(*blocks)),
        Event::CrcVerified(crc) => format!("CRC-24 {} verified", hex(*crc)),
        Event::RestartFailed(error) => format!(
            "The restart command ended with an error ({error}); waiting for the firmware all the same"
        ),
        Event::StartingFirmware { timeout } => format!(
            "Starting the firmware, waiting up to {} s",
            timeout.as_secs()
        ),
        Event::FirmwareStarted(device) => {
            format!("Firmware started: {}", hid::describe(device))
        }
    }
}

fn error_chain(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

fn block_count(count: usize) -> String {
    if count == 1 {
        "1 block".to_owned()
    } else {
        format!("{count} blocks")
    }
}

fn hex(value: u32) -> String {
    format!("0x{:04x}_{:04x}", value >> 16, value & 0xffff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_groups_four_digits() {
        assert_eq!(hex(0x0800_8000), "0x0800_8000");
        assert_eq!(hex(0x00ab_cdef), "0x00ab_cdef");
    }

    #[test]
    fn block_count_names_one_block() {
        assert_eq!(block_count(1), "1 block");
        assert_eq!(block_count(3), "3 blocks");
    }

    #[test]
    fn progress_is_a_percentage_of_the_blocks() {
        assert!((Blocks { total: 4, sent: 1 }.percent() - 25.0).abs() < f32::EPSILON);
        assert!(Blocks { total: 0, sent: 0 }.percent().abs() < f32::EPSILON);
    }

    #[test]
    fn error_chain_joins_the_sources() {
        #[derive(Debug, thiserror::Error)]
        #[error("could not write the image")]
        struct Outer(#[source] std::io::Error);
        let error = Outer(std::io::Error::other("the device went away"));
        assert_eq!(
            error_chain(&error),
            "could not write the image: the device went away"
        );
    }
}
