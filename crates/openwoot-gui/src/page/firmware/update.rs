use std::error::Error;

use gpui_kit::{
    ClipboardItem, Context, Div, Entity, FontWeight, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        notification::Notification,
        progress::Progress,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_host::{
    flash::{Event, Plan},
    hid,
};

use crate::{
    page::settings::{Tone, callout},
    store::Store,
};

const PROGRESS_HEIGHT: f32 = 12.0;
const PROGRESS_WIDTH: f32 = 400.0;

pub(super) struct Update {
    store: Entity<Store>,
    plan: Plan,
    phase: Phase,
    stage: Stage,
    blocks: Option<Blocks>,
    restart_failed: bool,
    log: Vec<String>,
    _observation: Subscription,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    Starting,
    EnteringUpdateMode,
    WaitingForUpdateMode,
    Writing,
    WaitingForKeyboard,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    Ready,
    Running,
    Done,
    Failed(SharedString),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Blocks {
    total: usize,
    sent: usize,
}

impl Update {
    pub(super) fn new(store: Entity<Store>, plan: Plan, cx: &mut Context<Self>) -> Self {
        let observation = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            store,
            plan,
            phase: Phase::Ready,
            stage: Stage::Starting,
            blocks: None,
            restart_failed: false,
            log: Vec::new(),
            _observation: observation,
        }
    }

    pub(super) fn closable(&self) -> bool {
        self.phase != Phase::Running
    }

    fn install(&mut self, cx: &mut Context<Self>) {
        if self.phase == Phase::Running {
            return;
        }
        self.phase = Phase::Running;
        self.stage = Stage::Starting;
        self.blocks = None;
        self.restart_failed = false;
        self.log.clear();
        let (sender, events) = async_channel::unbounded();
        let installed = self
            .store
            .read(cx)
            .device()
            .install(self.plan.clone(), sender);
        let pending = events.clone();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this
                    .update(cx, |update, cx| {
                        update.record(&event);
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
            let outcome = installed.await;
            this.update(cx, |update, cx| {
                while let Ok(event) = pending.try_recv() {
                    update.record(&event);
                }
                update.phase = match outcome {
                    Ok(()) => {
                        update.stage = Stage::Complete;
                        Phase::Done
                    }
                    Err(error) => {
                        let reason = error_chain(&error);
                        update.log.push(reason.clone());
                        Phase::Failed(reason.into())
                    }
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn record(&mut self, event: &Event) {
        self.stage = self.stage.after(event);
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
            Event::RestartFailed(_) => self.restart_failed = true,
            Event::FirmwareIdentified { .. }
            | Event::EnteringUpdateMode { .. }
            | Event::UpdateModeReached(_)
            | Event::BootloaderReady(_)
            | Event::CrcVerified(_)
            | Event::StartingFirmware { .. }
            | Event::FirmwareStarted(_) => {}
        }
        self.log.push(step_text(event));
    }

    fn keyboard_name(&self, cx: &Context<Self>) -> SharedString {
        self.store.read(cx).keyboard().map_or_else(
            || SharedString::from(self.plan.board.clone()),
            |keyboard| keyboard.name.clone(),
        )
    }

    fn copy_details(&self, window: &mut Window, cx: &mut Context<Self>) {
        let mut text = format!("{}\n", self.plan.path.display());
        for line in &self.log {
            text.push_str(line);
            text.push('\n');
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        window.push_notification(Notification::info("Copied"), cx);
    }

    fn ready(&self, cx: &mut Context<Self>) -> Div {
        let store = self.store.read(cx);
        let installed = store
            .keyboard()
            .map(|keyboard| format!("openwoot {}", keyboard.version.firmware));
        let unsaved = store.keyboard().is_some_and(|keyboard| keyboard.unsaved);
        let mut rows = Vec::new();
        if let Some(installed) = installed {
            rows.push(("Installed", installed));
        }
        rows.push(("File", file_name(&self.plan)));
        rows.extend(plan_lines(&self.plan));
        let warning = if unsaved {
            "Unsaved changes are lost; keep the keyboard plugged in until the update completes."
        } else {
            "Keep the keyboard plugged in until the update completes."
        };
        v_flex()
            .gap_5()
            .child(title("Install firmware from a file", cx))
            .child(
                v_flex()
                    .gap_2()
                    .children(rows.into_iter().map(|(label, value)| row(label, value, cx))),
            )
            .child(callout(Tone::Info, warning, cx))
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel-install")
                            .outline()
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("install-firmware")
                            .primary()
                            .label("Install firmware")
                            .on_click(cx.listener(|this, _, _, cx| this.install(cx))),
                    ),
            )
    }

    fn overall_percent(&self) -> f32 {
        let (start, end) = self.stage.span();
        let within = match (self.stage, self.blocks) {
            (Stage::Writing, Some(blocks)) => blocks.percent() / 100.0,
            _ => 0.0,
        };
        start + (end - start) * within
    }

    fn running(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let total = Stage::ALL.len();
        v_flex()
            .items_center()
            .gap_8()
            .py_2()
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .child(title(
                        format!("Updating your {}", self.keyboard_name(cx)),
                        cx,
                    ))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("It takes a moment. Do not unplug the keyboard."),
                    ),
            )
            .child(
                v_flex()
                    .w(px(PROGRESS_WIDTH))
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child(format!("Step {} of {total}", self.stage.number())),
                    )
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(self.stage.text()),
                    )
                    .child(
                        div().w_full().pt_2().child(
                            Progress::new("update-progress")
                                .h(px(PROGRESS_HEIGHT))
                                .value(self.overall_percent()),
                        ),
                    )
                    .when(self.restart_failed, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("The restart command failed; waiting for the keyboard."),
                        )
                    }),
            )
    }

    fn done(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let version = self
            .store
            .read(cx)
            .keyboard()
            .map(|keyboard| keyboard.version.firmware);
        v_flex()
            .items_center()
            .gap_4()
            .py_2()
            .child(
                Icon::new(Catalog::CircleCheck)
                    .large()
                    .text_color(theme.success),
            )
            .child(title(
                format!("Your {} is updated", self.keyboard_name(cx)),
                cx,
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(version.map_or_else(
                        || SharedString::from("The keyboard started the new firmware."),
                        |version| format!("It runs openwoot {version}.").into(),
                    )),
            )
            .child(
                h_flex().pt_2().child(
                    Button::new("update-done")
                        .primary()
                        .label("Done")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    }

    fn failed(&self, reason: &SharedString, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .child(Icon::new(Catalog::CircleX).text_color(theme.danger))
                    .child(title("The update did not complete", cx)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(format!(
                        "Stopped at step {} of {}: {}.",
                        self.stage.number(),
                        Stage::ALL.len(),
                        self.stage.text().to_lowercase()
                    )),
            )
            .child(
                div()
                    .p_3()
                    .rounded(theme.radius)
                    .bg(theme.group_box)
                    .text_sm()
                    .text_color(theme.danger)
                    .child(reason.clone()),
            )
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        Button::new("copy-update-details")
                            .ghost()
                            .icon(Catalog::Copy)
                            .label("Copy details")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.copy_details(window, cx)),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("close-update")
                                    .outline()
                                    .label("Close")
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child(
                                Button::new("retry-update")
                                    .primary()
                                    .label("Try again")
                                    .on_click(cx.listener(|this, _, _, cx| this.install(cx))),
                            ),
                    ),
            )
    }
}

impl Render for Update {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.phase.clone() {
            Phase::Ready => self.ready(cx),
            Phase::Running => self.running(cx),
            Phase::Done => self.done(cx),
            Phase::Failed(reason) => self.failed(&reason, cx),
        }
    }
}

pub(super) fn error_chain(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

impl Stage {
    const ALL: [Self; 6] = [
        Self::Starting,
        Self::EnteringUpdateMode,
        Self::WaitingForUpdateMode,
        Self::Writing,
        Self::WaitingForKeyboard,
        Self::Complete,
    ];

    const fn text(self) -> &'static str {
        match self {
            Self::Starting => "Initialising update",
            Self::EnteringUpdateMode => "Resetting the keyboard to update mode",
            Self::WaitingForUpdateMode => "Waiting for update mode",
            Self::Writing => "Writing firmware",
            Self::WaitingForKeyboard => "Waiting for the keyboard",
            Self::Complete => "Update complete",
        }
    }

    fn number(self) -> usize {
        Self::ALL
            .iter()
            .position(|stage| *stage == self)
            .map_or(Self::ALL.len(), |index| index + 1)
    }

    const fn span(self) -> (f32, f32) {
        match self {
            Self::Starting => (0.0, 2.0),
            Self::EnteringUpdateMode => (2.0, 6.0),
            Self::WaitingForUpdateMode => (6.0, 15.0),
            Self::Writing => (15.0, 90.0),
            Self::WaitingForKeyboard => (90.0, 100.0),
            Self::Complete => (100.0, 100.0),
        }
    }

    fn after(self, event: &Event) -> Self {
        let reached = match event {
            Event::FirmwareIdentified { .. } => Self::EnteringUpdateMode,
            Event::EnteringUpdateMode { .. } => Self::WaitingForUpdateMode,
            Event::UpdateModeReached(_)
            | Event::BootloaderReady(_)
            | Event::Writing { .. }
            | Event::Sent { .. } => Self::Writing,
            Event::CrcVerified(_) | Event::RestartFailed(_) | Event::StartingFirmware { .. } => {
                Self::WaitingForKeyboard
            }
            Event::FirmwareStarted(_) => Self::Complete,
        };
        self.max(reached)
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

fn title(text: impl Into<SharedString>, cx: &Context<Update>) -> Div {
    div()
        .text_xl()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
        .child(text.into())
}

fn row(label: &'static str, value: String, cx: &Context<Update>) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .gap_4()
        .justify_between()
        .text_sm()
        .child(div().text_color(theme.muted_foreground).child(label))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(theme.foreground)
                .child(value),
        )
}

fn file_name(plan: &Plan) -> String {
    plan.path.file_name().map_or_else(
        || plan.path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn plan_lines(plan: &Plan) -> Vec<(&'static str, String)> {
    let mut lines = vec![
        ("Board", plan.board.clone()),
        ("Size", format!("{} bytes", plan.image_len)),
    ];
    if let Some(update) = &plan.update {
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
            format!("found {}, {firmware}", hid::describe(device))
        }
        Event::EnteringUpdateMode { timeout } => format!(
            "asked the firmware to restart into update mode, waiting up to {} s",
            timeout.as_secs()
        ),
        Event::UpdateModeReached(device) => {
            format!("update mode reached: {}", hid::describe(device))
        }
        Event::BootloaderReady(state) => format!(
            "bootloader ready: {}, serial {}",
            state.version.as_ref().map_or_else(
                || "version not reported".to_owned(),
                |version| format!("version {version}")
            ),
            state.serial.as_deref().unwrap_or("not reported"),
        ),
        Event::Writing { blocks } => format!("writing {}", block_count(*blocks)),
        Event::Sent { blocks } => format!("{} written", block_count(*blocks)),
        Event::CrcVerified(crc) => format!("CRC-24 {} verified", hex(*crc)),
        Event::RestartFailed(error) => format!("the restart command failed: {error}"),
        Event::StartingFirmware { timeout } => format!(
            "starting the firmware, waiting up to {} s",
            timeout.as_secs()
        ),
        Event::FirmwareStarted(device) => {
            format!("firmware started: {}", hid::describe(device))
        }
    }
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
    use std::time::Duration;

    use super::*;

    #[test]
    fn events_only_move_the_stage_forward() {
        let stage = Stage::Starting
            .after(&Event::EnteringUpdateMode {
                timeout: Duration::from_secs(20),
            })
            .after(&Event::Writing { blocks: 4 });
        assert_eq!(stage, Stage::Writing);
        assert_eq!(
            stage.after(&Event::EnteringUpdateMode {
                timeout: Duration::from_secs(20)
            }),
            Stage::Writing
        );
        assert_eq!(
            stage.after(&Event::CrcVerified(1)),
            Stage::WaitingForKeyboard
        );
    }

    #[test]
    fn stages_count_from_one() {
        assert_eq!(Stage::Starting.number(), 1);
        assert_eq!(Stage::Complete.number(), 6);
    }

    #[test]
    fn stage_spans_fill_the_bar_from_the_left_without_gaps() {
        let mut reached = 0.0;
        for stage in Stage::ALL {
            let (start, end) = stage.span();
            assert!((start - reached).abs() < f32::EPSILON);
            assert!(end >= start);
            reached = end;
        }
        assert!((reached - 100.0).abs() < f32::EPSILON);
    }

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
