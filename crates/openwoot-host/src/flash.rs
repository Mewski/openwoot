use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use async_hid::{Device, DeviceInfo};
use openwoot_image::{Image, Region, UpdateMode, UpdateProtocol, UsbIds};

use crate::{
    bootloader::{self, Session, State, Upload},
    escape,
    hid::{self, Hid, Interface},
    image::{self, Content, Format},
    keyboard::{self, Firmware, Found, Identity, MODE_CHANGE_TIMEOUT, Running},
    transfer,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub path: PathBuf,
    pub board: String,
    pub image_len: usize,
    pub area: Region,
    pub ram: Region,
    pub usb: UsbIds,
    pub update: Option<Update>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub mode: UpdateMode,
    pub upload: Upload,
    pub identity: Identity,
}

#[derive(Debug, Clone)]
pub enum Event {
    FirmwareIdentified {
        device: DeviceInfo,
        firmware: Firmware,
    },
    EnteringUpdateMode {
        timeout: Duration,
    },
    UpdateModeReached(DeviceInfo),
    BootloaderReady(State),
    Writing {
        blocks: usize,
    },
    Sent {
        blocks: usize,
    },
    CrcVerified(u32),
    RestartFailed(transfer::Error),
    StartingFirmware {
        timeout: Duration,
    },
    FirmwareStarted(DeviceInfo),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read `{}`", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not parse `{}` as Intel HEX", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: image::Error,
    },
    #[error("`{}` is not an openwoot firmware image", path.display())]
    NotAnImage {
        path: PathBuf,
        #[source]
        source: openwoot_image::Error,
    },
    #[error("the image cannot be written through the factory bootloader; nothing was sent")]
    Unwritable(#[source] bootloader::Error),
    #[error(
        "the board `{}` has no known update mode; nothing was sent",
        escape::control_characters(board)
    )]
    NoUpdateMode { board: String },
    #[error(transparent)]
    Keyboard(#[from] keyboard::Error),
    #[error("could not ask the keyboard to enter update mode")]
    UpdateModeNotRequested(#[source] keyboard::Error),
    #[error("could not reach the keyboard in update mode; nothing was written")]
    UpdateModeUnreachable(#[source] keyboard::Error),
    #[error("could not query the bootloader; nothing was written")]
    BootloaderUnanswered(#[source] bootloader::Error),
    #[error(
        "could not write the image; the keyboard stays in update mode, run `openwoot flash` again"
    )]
    ImageNotWritten(#[source] bootloader::Error),
}

pub fn plan(path: &Path) -> Result<Plan, Error> {
    let file = fs::read(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })?;
    let content =
        Content::parse(&file, Format::detect(path, &file)).map_err(|source| Error::Parse {
            path: path.to_owned(),
            source,
        })?;
    let image = Image::new(&content.bytes, content.start).map_err(|source| Error::NotAnImage {
        path: path.to_owned(),
        source,
    })?;
    let descriptor = image.descriptor();
    let usb = descriptor.usb_ids();
    let update = descriptor
        .update_mode()
        .map(|mode| match mode.protocol {
            UpdateProtocol::WootingFactoryBootloader => {
                Upload::within(image.bytes(), mode.block_size, descriptor.image())
                    .map(|upload| Update {
                        mode,
                        upload,
                        identity: Identity::new(usb, mode),
                    })
                    .map_err(Error::Unwritable)
            }
        })
        .transpose()?;
    Ok(Plan {
        path: path.to_owned(),
        board: descriptor.board_name().unwrap_or_default().to_owned(),
        image_len: image.bytes().len(),
        area: descriptor.image(),
        ram: descriptor.ram(),
        usb,
        update,
    })
}

pub async fn install(hid: &Hid, plan: &Plan, mut events: impl FnMut(Event)) -> Result<(), Error> {
    let Some(update) = &plan.update else {
        return Err(Error::NoUpdateMode {
            board: plan.board.clone(),
        });
    };
    let identity = update.identity;
    let interface = match keyboard::find(hid, identity).await? {
        Found::UpdateMode(device) => Interface::open(&device)
            .await
            .map_err(|error| Error::UpdateModeUnreachable(error.into()))?,
        Found::Firmware(device) => {
            ask_for_update_mode(&device, &mut events)
                .await
                .map_err(Error::UpdateModeNotRequested)?;
            let (_, interface) = keyboard::open_update_mode(hid, identity)
                .await
                .map_err(Error::UpdateModeUnreachable)?;
            interface
        }
    };
    events(Event::UpdateModeReached(interface.info().clone()));

    let upload = &update.upload;
    let mut session = Session::new(hid::UpdateMode::new(interface));
    let state = session.state().await.map_err(Error::BootloaderUnanswered)?;
    events(Event::BootloaderReady(state));

    events(Event::Writing {
        blocks: upload.block_count(),
    });
    let written = session
        .install(upload, |sent| events(Event::Sent { blocks: sent }))
        .await;
    drop(session);
    match written {
        Ok(()) => events(Event::CrcVerified(upload.crc)),
        Err(bootloader::Error::Restart(error)) => {
            events(Event::CrcVerified(upload.crc));
            events(Event::RestartFailed(error));
        }
        Err(error) => return Err(Error::ImageNotWritten(error)),
    }

    events(Event::StartingFirmware {
        timeout: MODE_CHANGE_TIMEOUT,
    });
    let firmware = keyboard::wait_for_firmware(hid, identity).await?;
    events(Event::FirmwareStarted(firmware.to_device_info()));
    Ok(())
}

async fn ask_for_update_mode(
    device: &Device,
    events: &mut impl FnMut(Event),
) -> Result<(), keyboard::Error> {
    let firmware = Running::identify(Interface::open(device).await?).await?;
    events(Event::FirmwareIdentified {
        device: firmware.reports().info().clone(),
        firmware: firmware.firmware().clone(),
    });
    events(Event::EnteringUpdateMode {
        timeout: MODE_CHANGE_TIMEOUT,
    });
    firmware.enter_update_mode().await
}
