use std::{error::Error, path::PathBuf};

use gpui_kit::{
    Context, Div, Entity, FontWeight, IntoElement, ParentElement as _, PathPromptOptions, Render,
    SharedString, Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, WindowExt as _,
        button::{Button, ButtonVariant, ButtonVariants as _},
        h_flex,
        progress::Progress,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
};
use openwoot_host::{
    flash::{self, Event, Plan},
    hid,
};

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
        let title = format!("Install the firmware for `{}`?", plan.board);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            let plan = plan.clone();
            alert
                .title(title.clone())
                .description(
                    "The keyboard restarts into update mode, receives the image and restarts with \
                     the new firmware. It does not type until the update completes; keep it \
                     plugged in. If the update is interrupted, the keyboard starts in update mode \
                     and the image can be installed again from here. To return to the stock \
                     firmware, use Wootility.",
                )
                .confirm()
                .ok_text("Install firmware")
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

    fn connected_section(&self, cx: &Context<Self>) -> Div {
        let store = self.store.read(cx);
        let section = panel::section("Connected keyboard", None, cx);
        let Some(keyboard) = store.keyboard() else {
            let reason = match store.link() {
                Link::Unreachable(reason) => Some(reason.clone()),
                Link::Searching | Link::Loading | Link::Connected => None,
            };
            return section
                .child(panel::note(
                    "No keyboard with openwoot firmware is connected. A keyboard in update mode \
                     or running the stock firmware can still receive an openwoot image.",
                    cx,
                ))
                .children(reason.map(|reason| panel::note(reason, cx)));
        };
        section
            .child(panel::row(
                "Keyboard",
                None,
                div().text_sm().child(keyboard.name.clone()),
                cx,
            ))
            .child(panel::row(
                "Firmware",
                None,
                div()
                    .text_sm()
                    .child(format!("openwoot {}", keyboard.version.firmware)),
                cx,
            ))
            .child(panel::row(
                "Protocol",
                None,
                div()
                    .text_sm()
                    .child(format!("version {}", keyboard.version.protocol)),
                cx,
            ))
    }

    fn image_section(&self, cx: &mut Context<Self>) -> Div {
        let installing = self.installing();
        let mut section = panel::section(
            "Flash firmware image",
            Some(
                "Choose an openwoot firmware image (.hex or .bin). It is checked before anything \
                 is sent to the keyboard."
                    .into(),
            ),
            cx,
        )
        .child(
            h_flex().child(
                Button::new("choose-image")
                    .outline()
                    .icon(Icon::new(Catalog::FileUp))
                    .label("Choose image…")
                    .disabled(installing || matches!(self.image, Some(Image::Checking(_))))
                    .on_click(cx.listener(|_, _, _, cx| Self::choose_image(cx))),
            ),
        );
        match &self.image {
            None => {}
            Some(Image::Checking(path)) => {
                section = section.child(panel::note(format!("Checking `{}`…", path.display()), cx));
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
                let writable = plan.update.is_some();
                for (label, value) in plan_lines(plan) {
                    section =
                        section.child(panel::row(label, None, div().text_sm().child(value), cx));
                }
                section = section.child(
                    h_flex().justify_end().child(
                        Button::new("install")
                            .danger()
                            .icon(Icon::new(Catalog::Upload))
                            .label("Install firmware")
                            .disabled(!writable || installing)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.confirm_install(window, cx)),
                            ),
                    ),
                );
            }
        }
        section
    }

    fn installation_section(installation: &Installation, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let progress = installation.blocks.map(|blocks| {
            v_flex()
                .gap_1()
                .child(Progress::new("upload-progress").value(blocks.percent()))
                .child(panel::note(
                    format!("{} of {} written", blocks.sent, block_count(blocks.total)),
                    cx,
                ))
        });
        let outcome = installation.outcome.as_ref().map(|outcome| match outcome {
            Ok(()) => h_flex()
                .gap_2()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.success)
                .child(Icon::new(Catalog::ListChecks))
                .child("The firmware is installed and running"),
            Err(reason) => h_flex()
                .gap_2()
                .text_sm()
                .text_color(theme.danger)
                .child(reason.clone()),
        });
        panel::section("Update", None, cx)
            .child(
                v_flex()
                    .gap_1()
                    .children(installation.steps.iter().enumerate().map(|(index, step)| {
                        h_flex()
                            .gap_2()
                            .text_sm()
                            .child(
                                div()
                                    .w_6()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{}.", index + 1)),
                            )
                            .child(step.clone())
                    })),
            )
            .children(progress)
            .when(installation.outcome.is_none(), |this| {
                this.child(panel::note(
                    "Keep the keyboard plugged in until the update completes.",
                    cx,
                ))
            })
            .children(outcome)
    }
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
        let connected = self.connected_section(cx);
        let image = self.image_section(cx);
        let installation = self
            .installation
            .as_ref()
            .map(|installation| Self::installation_section(installation, cx));
        panel::page(
            "Firmware",
            "Install openwoot firmware images through the keyboard's factory bootloader.",
            cx,
        )
        .child(
            v_flex()
                .gap_4()
                .max_w_full()
                .child(connected)
                .child(image)
                .children(installation),
        )
    }
}

fn plan_lines(plan: &Plan) -> Vec<(&'static str, String)> {
    let mut lines = vec![
        ("Board", format!("`{}`", plan.board)),
        (
            "Image",
            format!(
                "`{}`, {} bytes at {}",
                plan.path.display(),
                plan.image_len,
                hex(plan.area.start)
            ),
        ),
        (
            "Image area",
            format!("{}-{}", hex(plan.area.start), hex(plan.area.end)),
        ),
        (
            "RAM",
            format!("{}-{}", hex(plan.ram.start), hex(plan.ram.end)),
        ),
        (
            "USB ids",
            format!(
                "{:04x}:{:04x}",
                plan.usb.vendor_id, plan.usb.product_id_base
            ),
        ),
    ];
    match &plan.update {
        None => lines.push(("Update mode", "not supported by this board".to_owned())),
        Some(update) => {
            lines.push((
                "Update mode",
                format!(
                    "{:04x}:{:04x}, {}",
                    plan.usb.vendor_id, update.mode.product_id, update.mode.protocol
                ),
            ));
            lines.push((
                "Upload",
                format!(
                    "{} of {} bytes, CRC-24 {}",
                    block_count(update.upload.block_count()),
                    update.upload.block_size,
                    hex(update.upload.crc)
                ),
            ));
        }
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
