use std::pin::pin;

use openwoot::usb::{
    Face, HostState, Identity, KeyboardFormat, Mode, PollingRate, Resources, Speed, build,
};
use openwoot_image::UsbIds;

use super::{
    configuration::{self, Configuration},
    wire::{Host, Outcome, setup},
};

pub(crate) const IDENTITY: Identity<'static> = Identity {
    ids: UsbIds {
        vendor_id: 0x31E3,
        product_id_base: 0x1400,
    },
    manufacturer: "Wooting",
    product: "Test keyboard",
    serial_number: Some("A01B"),
};

const GET_DESCRIPTOR: u8 = 0x06;
const DEVICE_TO_HOST: u8 = 0x80;
const INTERFACE_RECIPIENT: u8 = 0x01;
const VENDOR: u8 = 0x40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Enumeration {
    pub(crate) device: Vec<u8>,
    pub(crate) configuration: Vec<u8>,
    pub(crate) report_descriptors: Vec<Result<Vec<u8>, Outcome>>,
    pub(crate) product: Result<Vec<u8>, Outcome>,
    pub(crate) ms_os_string: Result<Vec<u8>, Outcome>,
    pub(crate) extended_compat_id: Result<Vec<u8>, Outcome>,
}

#[must_use]
pub(crate) fn faces() -> Vec<Face> {
    let mut faces = Vec::new();
    for mode in [Mode::Xinput, Mode::Gamepad, Mode::NoGamepad] {
        for keyboard in [KeyboardFormat::NKeyRollover, KeyboardFormat::Boot] {
            for speed in [Speed::High, Speed::Full] {
                for gamepad_polling in [
                    PollingRate::Every4Milliseconds,
                    PollingRate::Every1Millisecond,
                    PollingRate::Every125Microseconds,
                ] {
                    faces.push(Face {
                        mode,
                        keyboard,
                        speed,
                        gamepad_polling,
                    });
                }
            }
        }
    }
    faces
}

pub(crate) fn enumerate(face: Face) -> Result<Enumeration, Outcome> {
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let mut device = build(host.driver(), face, IDENTITY, &mut resources, &state);
    let mut run = pin!(device.usb.run());
    host.attach(run.as_mut());
    let mut get = |request_type: u8, value: u16, index: u16| {
        host.control_in(
            run.as_mut(),
            setup(request_type, GET_DESCRIPTOR, value, index, 0xFFFF),
        )
    };
    let device_descriptor = get(DEVICE_TO_HOST, 0x0100, 0)?;
    let configuration = get(DEVICE_TO_HOST, 0x0200, 0)?;
    let report_descriptors = (0..5)
        .map(|interface| get(DEVICE_TO_HOST | INTERFACE_RECIPIENT, 0x2200, interface))
        .collect();
    let product = get(DEVICE_TO_HOST, 0x0302, 0x0409);
    let ms_os_string = get(DEVICE_TO_HOST, 0x03EE, 0);
    let extended_compat_id =
        host.control_in(run.as_mut(), setup(DEVICE_TO_HOST | VENDOR, 0xB0, 0, 4, 40));
    Ok(Enumeration {
        device: device_descriptor,
        configuration,
        report_descriptors,
        product,
        ms_os_string,
        extended_compat_id,
    })
}

pub(crate) fn configuration(face: Face) -> Result<Configuration, String> {
    let enumeration = enumerate(face).map_err(|outcome| format!("{outcome:?}"))?;
    configuration::parse(&enumeration.configuration)
}
