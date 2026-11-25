mod status;

use std::{
    fmt::Display,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use anyhow::{Context, Result};
use async_io::block_on;
use clap::{Args, Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use openwoot_host::{
    Connection, escape,
    flash::{self, Event, Plan},
    hid::{self, Hid},
    keyboard,
};
use openwoot_image::Hex;
use openwoot_protocol::{
    PROTOCOL_VERSION,
    endpoints::{GetDiagnostics, GetStorageUsage},
};
use status::Status;

const PROGRAM: &str = "openwoot";
const LABEL_WIDTH: usize = 13;
const PROGRESS_TEMPLATE: &str = "{bar:40} {pos}/{len} blocks, {elapsed}";
const NOT_REPORTED: &str = "not reported";

#[derive(Parser)]
#[command(name = PROGRAM, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Print the firmware and protocol version of the connected keyboard")]
    Version,
    #[command(about = "Print the diagnostic counters and states of the connected keyboard")]
    Status,
    #[command(about = "Write a firmware image to the keyboard through its factory bootloader")]
    Flash(FlashArgs),
}

#[derive(Args)]
struct FlashArgs {
    #[arg(help = "Intel HEX file (`.hex`) or flat binary (`.bin`) of a firmware build")]
    image: PathBuf,
    #[arg(
        long,
        help = "Check the image and print what would be sent, without touching a device"
    )]
    dry_run: bool,
}

#[derive(Default)]
struct Console {
    progress: Option<ProgressBar>,
    failure: Option<anyhow::Error>,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            writeln!(io::stderr().lock(), "{PROGRAM}: {error:#}").ok();
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Version => block_on(version()),
        Command::Status => block_on(status()),
        Command::Flash(args) => flash(&args),
    }
}

async fn version() -> Result<()> {
    let (device, version) = keyboard::firmware_version(&Hid::default())
        .await
        .context("could not read the firmware version")?;
    show("device", hid::describe(&device))?;
    show("firmware", version.firmware)?;
    if version.protocol == PROTOCOL_VERSION {
        show("protocol", version.protocol)
    } else {
        show(
            "protocol",
            format!("{} (this tool speaks {PROTOCOL_VERSION})", version.protocol),
        )
    }
}

async fn status() -> Result<()> {
    let mut connection = Connection::open(&Hid::default())
        .await
        .context("could not read the diagnostics")?;
    let report = connection
        .request::<GetDiagnostics>(&())
        .await
        .context("could not read the diagnostics")?;
    let storage_usage = connection
        .request::<GetStorageUsage>(&())
        .await
        .context("could not read the storage usage")?;
    let status = Status {
        device: hid::describe(connection.info()),
        report,
        storage_usage,
    };
    write!(io::stdout().lock(), "{status}").context("could not write to stdout")
}

fn flash(args: &FlashArgs) -> Result<()> {
    let plan = flash::plan(&args.image)?;
    show("board", &plan.board)?;
    show(
        "image",
        format!(
            "`{}`, {} bytes at {}",
            plan.path.display(),
            plan.image_len,
            Hex(plan.area.start)
        ),
    )?;
    show(
        "ram",
        format!("{}-{}", Hex(plan.ram.start), Hex(plan.ram.end)),
    )?;
    show(
        "usb ids",
        format!(
            "{:04x}:{:04x}",
            plan.usb.vendor_id, plan.usb.product_id_base
        ),
    )?;
    match &plan.update {
        None => {
            show("update mode", "not supported by this board")?;
            if args.dry_run {
                return Ok(());
            }
        }
        Some(update) => {
            show(
                "update mode",
                format!(
                    "{:04x}:{:04x}, {}",
                    plan.usb.vendor_id, update.mode.product_id, update.mode.protocol
                ),
            )?;
            show(
                "upload",
                format!(
                    "{} of {} bytes, CRC-24 {}",
                    blocks(update.upload.block_count()),
                    update.upload.block_size,
                    Hex(update.upload.crc),
                ),
            )?;
            if args.dry_run {
                return show("dry run", "nothing was sent");
            }
        }
    }
    block_on(install(&plan))
}

async fn install(plan: &Plan) -> Result<()> {
    let mut console = Console::default();
    let installed = flash::install(&Hid::default(), plan, |event| console.render(event)).await;
    console.clear_progress();
    installed?;
    console.failure.map_or(Ok(()), Err)
}

impl Console {
    fn render(&mut self, event: Event) {
        if let Err(error) = self.show(event) {
            self.failure.get_or_insert(error);
        }
    }

    fn show(&mut self, event: Event) -> Result<()> {
        match event {
            Event::FirmwareIdentified { device, firmware } => show(
                "firmware",
                format!("{}, {firmware}", hid::describe(&device)),
            ),
            Event::EnteringUpdateMode { timeout } => note(format!(
                "asking the firmware to reset into update mode, waiting up to {} s",
                timeout.as_secs()
            )),
            Event::UpdateModeReached(device) => show("device", hid::describe(&device)),
            Event::BootloaderReady(state) => {
                show(
                    "flags",
                    state
                        .flags
                        .map_or_else(not_reported, |flags| flags.to_string()),
                )?;
                show(
                    "bootloader",
                    state
                        .version
                        .map_or_else(not_reported, |version| format!("version {version}")),
                )?;
                show("serial", state.serial.unwrap_or_else(not_reported))?;
                show(
                    "session crc",
                    state
                        .session_crc
                        .map_or_else(not_reported, |crc| Hex(crc).to_string()),
                )
            }
            Event::Writing { blocks: count } => {
                let noted = note(format!(
                    "writing {}; until the update completes the keyboard starts in update mode",
                    blocks(count)
                ));
                self.progress = Some(progress_bar(count));
                noted
            }
            Event::Sent { blocks: sent } => {
                if let Some(progress) = &self.progress {
                    progress.set_position(u64::try_from(sent).unwrap_or(u64::MAX));
                }
                Ok(())
            }
            Event::CrcVerified(crc) => {
                self.clear_progress();
                show("crc verified", Hex(crc))
            }
            Event::RestartFailed(error) => note(format!(
                "the restart command ended with an error ({error}); waiting for the firmware all \
                 the same"
            )),
            Event::StartingFirmware { timeout } => note(format!(
                "starting the firmware, waiting up to {} s",
                timeout.as_secs()
            )),
            Event::FirmwareStarted(device) => show("firmware", hid::describe(&device)),
        }
    }

    fn clear_progress(&mut self) {
        if let Some(progress) = self.progress.take() {
            progress.finish_and_clear();
        }
    }
}

fn blocks(count: usize) -> String {
    if count == 1 {
        "1 block".to_owned()
    } else {
        format!("{count} blocks")
    }
}

fn not_reported() -> String {
    NOT_REPORTED.to_owned()
}

fn progress_bar(count: usize) -> ProgressBar {
    let style = ProgressStyle::with_template(PROGRESS_TEMPLATE)
        .unwrap_or_else(|_| ProgressStyle::default_bar());
    ProgressBar::new(u64::try_from(count).unwrap_or(u64::MAX)).with_style(style)
}

fn show(label: &str, value: impl Display) -> Result<()> {
    let value = escape::control_characters(&value.to_string());
    writeln!(
        io::stdout().lock(),
        "{:<LABEL_WIDTH$} {value}",
        format!("{label}:")
    )
    .context("could not write to stdout")
}

fn note(text: impl Display) -> Result<()> {
    writeln!(io::stderr().lock(), "{text}").context("could not write to stderr")
}
