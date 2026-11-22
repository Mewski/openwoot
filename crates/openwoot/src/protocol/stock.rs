use openwoot_protocol::stock::{
    COMMAND_COUNT, Command, FIRST_CURRENT_COMMAND, Request, Status, encode_reply,
};

use super::{
    Answer,
    requests::Queued,
    serial::{LEGACY_LEN, SerialNumber},
};
use crate::usb::{Face, KeyboardFormat, PollingRate, Speed};

const VERSION: [u8; 3] = [2, 1, 2];
const DEVICE_SETTINGS_LEN: usize = 7;
const READ_UNPROTECTED: u8 = 0x11;
const FORCE_FULL_SPEED: u8 = 1 << 3;
const SIX_KEY_ROLLOVER: u8 = 1 << 4;
const USB_MODE_SHIFT: u8 = 1;
const GAMEPAD_POLLING_SHIFT: u8 = 3;
const MCU_ID_LEN: usize = 12;
const FLASH_IDS_LEN: usize = 10;
const SERIAL_BLANK: i16 = -2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StockIdentity {
    pub serial: Option<&'static str>,
    pub face: Face,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Responder {
    serial: Option<SerialNumber>,
    device_settings: [u8; DEVICE_SETTINGS_LEN],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SerialSelector {
    Legacy = 0,
    McuId = 1,
    Message = 2,
    FlashIds = 3,
}

impl Responder {
    pub(crate) fn new(identity: StockIdentity) -> Self {
        Self {
            serial: identity.serial.and_then(SerialNumber::parse),
            device_settings: device_settings(identity.face),
        }
    }

    pub(crate) fn answer<'a>(&self, queued: Queued, report: &'a mut [u8]) -> Answer<'a> {
        let request = match queued {
            Queued::Status { command, status } => return reply(command, status, &[], report),
            Queued::Command(request) => request,
        };
        let command = request.command;
        let [low, ..] = request.argument.to_le_bytes();
        match Command::from_number(command) {
            Some(Command::Ping) => reply(command, Status::Success, &[!low], report),
            Some(Command::Version) => reply(command, Status::Success, &VERSION, report),
            Some(Command::EnterUpdateMode) => Answer {
                reply: None,
                update_mode: Some(low),
            },
            Some(Command::Serial) => self.serial(request, report),
            Some(Command::DeviceSettings) => {
                reply(command, Status::Success, &self.device_settings, report)
            }
            None if (FIRST_CURRENT_COMMAND..COMMAND_COUNT).contains(&command) => {
                reply(command, Status::Unsupported, &[], report)
            }
            None => reply(command, Status::Unknown, &[], report),
        }
    }

    fn serial<'a>(&self, request: Request, report: &'a mut [u8]) -> Answer<'a> {
        let command = request.command;
        let selector = [
            SerialSelector::Legacy,
            SerialSelector::McuId,
            SerialSelector::Message,
            SerialSelector::FlashIds,
        ]
        .into_iter()
        .find(|selector| *selector as u32 == request.argument);
        match (selector, self.serial) {
            (Some(SerialSelector::Legacy), serial) => {
                let legacy = serial.map_or([0; LEGACY_LEN], SerialNumber::to_legacy);
                reply(command, Status::Success, &legacy, report)
            }
            (Some(SerialSelector::McuId), _) => {
                reply(command, Status::Success, &[0; MCU_ID_LEN], report)
            }
            (Some(SerialSelector::Message), Some(serial)) => {
                reply(command, Status::Success, serial.encode().as_bytes(), report)
            }
            (Some(SerialSelector::Message), None) => {
                reply(command, Status::Error, &SERIAL_BLANK.to_le_bytes(), report)
            }
            (Some(SerialSelector::FlashIds), _) => {
                reply(command, Status::Success, &[0; FLASH_IDS_LEN], report)
            }
            (None, _) => reply(command, Status::Error, &[], report),
        }
    }
}

pub(crate) const fn accepts(command: u8) -> bool {
    command < COMMAND_COUNT
        && (command >= FIRST_CURRENT_COMMAND || Command::from_number(command).is_some())
}

fn reply<'a>(command: u8, status: Status, payload: &[u8], report: &'a mut [u8]) -> Answer<'a> {
    Answer {
        reply: encode_reply(command, status, payload, report).ok(),
        update_mode: None,
    }
}

fn device_settings(face: Face) -> [u8; DEVICE_SETTINGS_LEN] {
    let speed = match face.speed {
        Speed::Full => FORCE_FULL_SPEED,
        Speed::High => 0,
    };
    let rollover = match face.keyboard {
        KeyboardFormat::Boot => SIX_KEY_ROLLOVER,
        KeyboardFormat::NKeyRollover => 0,
    };
    let polling: u8 = match face.gamepad_polling {
        PollingRate::Every4Milliseconds => 0,
        PollingRate::Every1Millisecond => 1,
        PollingRate::Every125Microseconds => 2,
    };
    let usb_mode = u8::try_from(face.mode.product_id_offset()).unwrap_or_default();
    [
        speed | rollover,
        0,
        0,
        0,
        READ_UNPROTECTED,
        0,
        (usb_mode << USB_MODE_SHIFT) | (polling << GAMEPAD_POLLING_SHIFT),
    ]
}
