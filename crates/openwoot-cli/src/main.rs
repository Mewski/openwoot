use std::{
    fmt::Display,
    fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use anyhow::{Context, Result, bail};
use async_hid::Device;
use async_io::block_on;
use clap::{Args, Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use openwoot_cli::{
    bootloader::{self, Session, Upload},
    escape,
    hid::{self, Hid, Interface, UpdateMode},
    image::{Content, Format},
    keyboard::{self, Found, Identity, MODE_CHANGE_TIMEOUT, Running},
    status::Status,
};
use openwoot_image::{Hex, Image, UpdateProtocol};
use openwoot_protocol::PROTOCOL_VERSION;

const PROGRAM: &str = "openwoot";
const LABEL_WIDTH: usize = 13;
const PROGRESS_TEMPLATE: &str = "{bar:40} {pos}/{len} blocks, {elapsed}";
const NOT_REPORTED: &str = "not reported";
const NOTHING_WRITTEN: &str = "could not reach the keyboard in update mode; nothing was written";
const STAYS_IN_UPDATE_MODE: &str =
    "could not write the image; the keyboard stays in update mode, run `openwoot flash` again";

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
    let (device, report) = keyboard::diagnostics(&Hid::default())
        .await
        .context("could not read the diagnostics")?;
    let status = Status {
        device: hid::describe(&device),
        report,
    };
    write!(io::stdout().lock(), "{status}").context("could not write to stdout")
}

fn flash(args: &FlashArgs) -> Result<()> {
    let path = args.image.display();
    let file = fs::read(&args.image).with_context(|| format!("could not read `{path}`"))?;
    let content = Content::parse(&file, Format::detect(&args.image, &file))
        .with_context(|| format!("could not parse `{path}` as Intel HEX"))?;
    let image = Image::new(&content.bytes, content.start)
        .with_context(|| format!("`{path}` is not an openwoot firmware image"))?;
    let descriptor = image.descriptor();
    let usb = descriptor.usb_ids();
    let board = descriptor.board_name().unwrap_or_default();

    show("board", board)?;
    show(
        "image",
        format!(
            "`{path}`, {} bytes at {}",
            image.bytes().len(),
            Hex(descriptor.image().start)
        ),
    )?;
    let ram = descriptor.ram();
    show("ram", format!("{}-{}", Hex(ram.start), Hex(ram.end)))?;
    show(
        "usb ids",
        format!("{:04x}:{:04x}", usb.vendor_id, usb.product_id_base),
    )?;
    let Some(mode) = descriptor.update_mode() else {
        show("update mode", "not supported by this board")?;
        if args.dry_run {
            return Ok(());
        }
        bail!(
            "the board `{}` has no known update mode; nothing was sent",
            escape::control_characters(board)
        );
    };
    show(
        "update mode",
        format!(
            "{:04x}:{:04x}, {}",
            usb.vendor_id, mode.product_id, mode.protocol
        ),
    )?;
    match mode.protocol {
        UpdateProtocol::WootingFactoryBootloader => {
            let upload = Upload::within(image.bytes(), mode.block_size, descriptor.image())
                .context(
                    "the image cannot be written through the factory bootloader; nothing was sent",
                )?;
            show(
                "upload",
                format!(
                    "{} of {} bytes, CRC-24 {}",
                    blocks(upload.block_count()),
                    upload.block_size,
                    Hex(upload.crc),
                ),
            )?;
            if args.dry_run {
                return show("dry run", "nothing was sent");
            }
            block_on(install(Identity::new(usb, mode), &upload))
        }
    }
}

async fn install(identity: Identity, upload: &Upload) -> Result<()> {
    let hid = Hid::default();
    let (device, interface) = match keyboard::find(&hid, identity).await? {
        Found::UpdateMode(device) => {
            let interface = Interface::open(&device).await.context(NOTHING_WRITTEN)?;
            (device.to_device_info(), interface)
        }
        Found::Firmware(device) => {
            ask_for_update_mode(&device)
                .await
                .context("could not ask the keyboard to enter update mode")?;
            keyboard::open_update_mode(&hid, identity)
                .await
                .context(NOTHING_WRITTEN)?
        }
    };
    show("device", hid::describe(&device))?;

    let mut session = Session::new(UpdateMode::new(interface));
    let state = session
        .state()
        .await
        .context("could not query the bootloader; nothing was written")?;
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
    )?;

    note(format!(
        "writing {}; until the update completes the keyboard starts in update mode",
        blocks(upload.block_count())
    ))?;
    let progress = progress_bar(upload.block_count());
    let written = session
        .install(upload, |sent| {
            progress.set_position(u64::try_from(sent).unwrap_or(u64::MAX));
        })
        .await;
    progress.finish_and_clear();
    drop(session);
    match written {
        Ok(()) => show("crc verified", Hex(upload.crc))?,
        Err(bootloader::Error::Restart(error)) => {
            show("crc verified", Hex(upload.crc))?;
            note(format!(
                "the restart command ended with an error ({error}); waiting for the firmware all \
                 the same"
            ))?;
        }
        Err(error) => return Err(error).context(STAYS_IN_UPDATE_MODE),
    }

    note(format!(
        "starting the firmware, waiting up to {} s",
        MODE_CHANGE_TIMEOUT.as_secs()
    ))?;
    let firmware = keyboard::wait_for_firmware(&hid, identity).await?;
    show("firmware", hid::describe(&firmware))
}

async fn ask_for_update_mode(device: &Device) -> Result<()> {
    let firmware = Running::identify(Interface::open(device).await?).await?;
    show(
        "firmware",
        format!(
            "{}, {}",
            hid::describe(firmware.reports().info()),
            firmware.firmware()
        ),
    )?;
    note(format!(
        "asking the firmware to reset into update mode, waiting up to {} s",
        MODE_CHANGE_TIMEOUT.as_secs()
    ))?;
    Ok(firmware.enter_update_mode().await?)
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
