use std::{fmt::Display, fs, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result, bail};
use async_io::block_on;
use clap::{Args, Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use openwoot_cli::{
    bootloader::{Session, Upload},
    hid::{self, Hid, Interface, UpdateMode},
    image::Content,
    keyboard::{self, Identity, MODE_CHANGE_TIMEOUT, Running},
};
use openwoot_image::{Hex, Image};

const PROGRAM: &str = "openwoot";
const LABEL_WIDTH: usize = 13;
const PROGRESS_TEMPLATE: &str = "{bar:40} {pos}/{len} blocks, {elapsed}";
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
            eprintln!("{PROGRAM}: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Version => block_on(version()),
        Command::Flash(args) => flash(&args),
    }
}

async fn version() -> Result<()> {
    let (device, version) = keyboard::firmware_version(&Hid::default())
        .await
        .context("could not read the firmware version")?;
    show("device", hid::describe(&device));
    show("firmware", keyboard::format_version(&version));
    show("protocol", version.protocol);
    Ok(())
}

fn flash(args: &FlashArgs) -> Result<()> {
    let path = args.image.display();
    let file = fs::read(&args.image).with_context(|| format!("could not read `{path}`"))?;
    let content =
        Content::parse(&file).with_context(|| format!("could not parse `{path}` as Intel HEX"))?;
    let image = Image::new(&content.bytes, content.start)
        .with_context(|| format!("`{path}` is not an openwoot firmware image"))?;
    let descriptor = image.descriptor();
    let usb = descriptor.usb_ids();
    let board = descriptor.board_name().unwrap_or_default();

    show("board", board);
    show(
        "image",
        format!(
            "`{path}`, {} bytes at {}",
            image.bytes().len(),
            Hex(descriptor.image().start)
        ),
    );
    show(
        "usb ids",
        format!("{:04x}:{:04x}", usb.vendor_id, usb.product_id_base),
    );
    let Some(mode) = descriptor.update_mode() else {
        show("update mode", "not supported by this board");
        if args.dry_run {
            return Ok(());
        }
        bail!("the {board} has no known update mode; nothing was sent");
    };
    let block_size = usize::try_from(mode.block_size).context("block size exceeds memory")?;
    let upload = Upload::new(image.bytes(), block_size);
    show(
        "update mode",
        format!(
            "{:04x}:{:04x}, {} blocks of {} bytes, CRC-24 {}",
            usb.vendor_id,
            mode.product_id,
            upload.block_count(),
            upload.block_size,
            Hex(upload.crc),
        ),
    );
    if args.dry_run {
        show("dry run", "nothing was sent");
        return Ok(());
    }
    block_on(install(Identity::new(usb, mode), &upload))
}

async fn install(identity: Identity, upload: &Upload) -> Result<()> {
    let hid = Hid::default();
    let device = if let Some(device) = keyboard::find_update_mode(&hid, identity).await? {
        device
    } else {
        let asked = async {
            let firmware =
                Running::identify(keyboard::open_firmware(&hid, identity).await?).await?;
            show(
                "firmware",
                format!(
                    "{}, {}",
                    hid::describe(firmware.reports().info()),
                    firmware.firmware()
                ),
            );
            note(format!(
                "asking the firmware to reset into update mode, waiting up to {} s",
                MODE_CHANGE_TIMEOUT.as_secs()
            ));
            firmware.enter_update_mode().await
        };
        asked
            .await
            .context("could not ask the keyboard to enter update mode")?;
        keyboard::wait_for_update_mode(&hid, identity).await?
    };
    show("device", hid::describe(&device));

    let mut session = Session::new(UpdateMode::new(Interface::open(&device).await?));
    let state = session
        .state()
        .await
        .context("could not query the bootloader; nothing was written")?;
    show(
        "flags",
        state
            .flags
            .map_or_else(|| "not reported".to_owned(), |flags| flags.to_string()),
    );
    show("bootloader", format!("version {}", state.version));
    show("serial", state.serial);
    show("session crc", Hex(state.session_crc));

    note(format!(
        "writing {} blocks; until the update completes the keyboard starts in update mode",
        upload.block_count()
    ));
    let progress = progress_bar(upload.block_count());
    let written = session
        .install(upload, |blocks| {
            progress.set_position(u64::try_from(blocks).unwrap_or(u64::MAX));
        })
        .await;
    progress.finish_and_clear();
    written.context(STAYS_IN_UPDATE_MODE)?;
    show("crc verified", Hex(upload.crc));
    drop(session);

    note(format!(
        "starting the firmware, waiting up to {} s",
        MODE_CHANGE_TIMEOUT.as_secs()
    ));
    let firmware = keyboard::wait_for_firmware(&hid, identity).await?;
    show("firmware", hid::describe(&firmware));
    Ok(())
}

fn progress_bar(blocks: usize) -> ProgressBar {
    let style = ProgressStyle::with_template(PROGRESS_TEMPLATE)
        .unwrap_or_else(|_| ProgressStyle::default_bar());
    ProgressBar::new(u64::try_from(blocks).unwrap_or(u64::MAX)).with_style(style)
}

fn show(label: &str, value: impl Display) {
    println!("{:<LABEL_WIDTH$} {value}", format!("{label}:"));
}

fn note(text: impl Display) {
    eprintln!("{text}");
}
