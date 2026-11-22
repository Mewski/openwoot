use std::{fs, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use openwoot_cli::{bootloader::Upload, image::Content};
use openwoot_image::Image;

const LABEL_WIDTH: usize = 10;

#[derive(Parser)]
#[command(version, about)]
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
    #[arg(help = "Intel HEX file (.hex) or flat binary (.bin) of a firmware build")]
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
            eprintln!("openwoot: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Version => {
            bail!(
                "reading the version needs the configuration channel, which is not implemented yet"
            )
        }
        Command::Flash(args) => flash(&args),
    }
}

fn flash(args: &FlashArgs) -> Result<()> {
    let path = args.image.display();
    let file = fs::read(&args.image).with_context(|| format!("could not read `{path}`"))?;
    let content =
        Content::parse(&file).with_context(|| format!("`{path}` is not valid Intel HEX"))?;
    let image = Image::new(&content.bytes, content.start)
        .with_context(|| format!("`{path}` is not an openwoot firmware image"))?;
    let descriptor = image.descriptor();
    let region = descriptor.image();
    let usb = descriptor.usb_ids();
    let board = descriptor.board_name().unwrap_or_default();

    show("board", board);
    show(
        "image",
        &format!(
            "{path}, {} bytes at {:#010x}",
            image.bytes().len(),
            region.start
        ),
    );
    show(
        "usb ids",
        &format!("{:04x}:{:04x}", usb.vendor_id, usb.product_id_base),
    );
    let Some(mode) = descriptor.update_mode() else {
        show("update", "not supported by this board");
        if args.dry_run {
            return Ok(());
        }
        bail!("the {board} has no known update mode; nothing was sent");
    };
    let block_size = usize::try_from(mode.block_size).context("block size exceeds memory")?;
    let upload = Upload::new(image.bytes(), block_size);
    show(
        "update",
        &format!(
            "product id {:04x}, {} blocks of {} bytes, CRC-24 {:#08x}",
            mode.product_id,
            upload.block_count(),
            upload.block_size,
            upload.crc,
        ),
    );
    if args.dry_run {
        show("dry run", "nothing was sent");
        return Ok(());
    }
    bail!("flashing through the bootloader is not implemented yet; --dry-run checks an image")
}

fn show(label: &str, value: &str) {
    println!("{label:<LABEL_WIDTH$}: {value}");
}
