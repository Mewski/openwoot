use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use embassy_usb::{
    Handler,
    control::{InResponse, OutResponse, Recipient, Request, RequestType},
    types::StringIndex,
};

use super::xinput;
use crate::hid::Leds;

pub(super) const INTERFACE_COUNT: usize = 5;
pub(super) const KEYBOARD_INTERFACE: usize = 1;
pub(super) const HID_DESCRIPTOR_TYPE: u8 = 0x21;
pub(super) const REPORT_DESCRIPTOR_TYPE: u8 = 0x22;

const GET_IDLE: u8 = 0x02;
const GET_PROTOCOL: u8 = 0x03;
const SET_REPORT: u8 = 0x09;
const SET_IDLE: u8 = 0x0A;
const SET_PROTOCOL: u8 = 0x0B;
const OUTPUT_REPORT_TYPE: u8 = 0x02;
const HID_DESCRIPTOR_LEN: u8 = 9;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Protocol {
    Boot,
    #[default]
    Report,
}

#[derive(Debug, Default)]
pub struct HostState {
    keyboard_leds: AtomicU8,
    keyboard_boot_protocol: AtomicBool,
}

#[derive(Debug)]
pub struct Control<'d> {
    host: &'d HostState,
    interfaces: [Option<HidInterface>; INTERFACE_COUNT],
    xinput: bool,
}

#[derive(Debug, Clone, Copy)]
struct HidInterface {
    report_descriptor: &'static [u8],
    idle: u8,
    protocol: Protocol,
}

impl HostState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            keyboard_leds: AtomicU8::new(0),
            keyboard_boot_protocol: AtomicBool::new(false),
        }
    }

    pub fn keyboard_leds(&self) -> Leds {
        Leds::from_bits(self.keyboard_leds.load(Ordering::Relaxed))
    }

    pub fn keyboard_protocol(&self) -> Protocol {
        if self.keyboard_boot_protocol.load(Ordering::Relaxed) {
            Protocol::Boot
        } else {
            Protocol::Report
        }
    }

    fn set_keyboard_protocol(&self, protocol: Protocol) {
        self.keyboard_boot_protocol
            .store(protocol == Protocol::Boot, Ordering::Relaxed);
    }

    fn receive_report(&self, interface: usize, report_type: u8, data: &[u8]) -> OutResponse {
        match (interface, report_type, data) {
            (KEYBOARD_INTERFACE, OUTPUT_REPORT_TYPE, [leds, ..]) => {
                self.keyboard_leds.store(*leds, Ordering::Relaxed);
                OutResponse::Accepted
            }
            _ => OutResponse::Rejected,
        }
    }
}

impl<'d> Control<'d> {
    pub(super) const fn new(host: &'d HostState, xinput: bool) -> Self {
        Self {
            host,
            interfaces: [None; INTERFACE_COUNT],
            xinput,
        }
    }

    pub(super) fn add_hid_interface(&mut self, number: usize, report_descriptor: &'static [u8]) {
        if let Some(interface) = self.interfaces.get_mut(number) {
            *interface = Some(HidInterface {
                report_descriptor,
                idle: 0,
                protocol: Protocol::Report,
            });
        }
    }

    fn hid_interface(&mut self, request: &Request) -> Option<(usize, &mut HidInterface)> {
        if request.recipient != Recipient::Interface {
            return None;
        }
        let number = usize::from(request.index);
        let interface = self.interfaces.get_mut(number)?.as_mut()?;
        Some((number, interface))
    }
}

impl Handler for Control<'_> {
    fn reset(&mut self) {
        for interface in self.interfaces.iter_mut().flatten() {
            interface.idle = 0;
            interface.protocol = Protocol::Report;
        }
        self.host.set_keyboard_protocol(Protocol::Report);
    }

    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        if req.request_type != RequestType::Class {
            return None;
        }
        let host = self.host;
        let (number, interface) = self.hid_interface(&req)?;
        let [high, low] = req.value.to_be_bytes();
        match req.request {
            SET_IDLE => {
                interface.idle = high;
                Some(OutResponse::Accepted)
            }
            SET_PROTOCOL => {
                let protocol = if low == 0 {
                    Protocol::Boot
                } else {
                    Protocol::Report
                };
                interface.protocol = protocol;
                if number == KEYBOARD_INTERFACE {
                    host.set_keyboard_protocol(protocol);
                }
                Some(OutResponse::Accepted)
            }
            SET_REPORT => Some(host.receive_report(number, high, data)),
            _ => Some(OutResponse::Rejected),
        }
    }

    fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        if req.request_type == RequestType::Vendor {
            let response = if req.request == xinput::MS_OS_VENDOR_CODE
                && req.index == xinput::EXTENDED_COMPAT_ID_INDEX
            {
                InResponse::Accepted(&xinput::EXTENDED_COMPAT_ID)
            } else {
                InResponse::Rejected
            };
            return self.xinput.then_some(response);
        }
        let [high, _] = req.value.to_be_bytes();
        let (_, interface) = self.hid_interface(&req)?;
        let response = match (req.request_type, req.request) {
            (RequestType::Standard, Request::GET_DESCRIPTOR) => match high {
                REPORT_DESCRIPTOR_TYPE => InResponse::Accepted(interface.report_descriptor),
                HID_DESCRIPTOR_TYPE => {
                    let mut descriptor = [0; HID_DESCRIPTOR_LEN as usize];
                    if let Some((head, body)) = descriptor.split_first_chunk_mut::<2>() {
                        *head = [HID_DESCRIPTOR_LEN, HID_DESCRIPTOR_TYPE];
                        body.copy_from_slice(&hid_class_descriptor(interface.report_descriptor));
                    }
                    copy_into(buf, &descriptor)
                }
                _ => InResponse::Rejected,
            },
            (RequestType::Class, GET_IDLE) => copy_into(buf, &[interface.idle]),
            (RequestType::Class, GET_PROTOCOL) => {
                copy_into(buf, &[u8::from(interface.protocol == Protocol::Report)])
            }
            _ => InResponse::Rejected,
        };
        Some(response)
    }

    fn get_string(&mut self, index: StringIndex, _lang_id: u16) -> Option<&str> {
        (self.xinput && u8::from(index) == xinput::MS_OS_STRING_INDEX)
            .then_some(xinput::MS_OS_STRING)
    }
}

pub(super) fn hid_class_descriptor(report_descriptor: &[u8]) -> [u8; 7] {
    let [length_low, length_high] = u16::try_from(report_descriptor.len())
        .unwrap_or(u16::MAX)
        .to_le_bytes();
    [
        0x11,
        0x01,
        0x00,
        0x01,
        REPORT_DESCRIPTOR_TYPE,
        length_low,
        length_high,
    ]
}

fn copy_into<'a>(buf: &'a mut [u8], bytes: &[u8]) -> InResponse<'a> {
    match buf.get_mut(..bytes.len()) {
        Some(response) => {
            response.copy_from_slice(bytes);
            InResponse::Accepted(response)
        }
        None => InResponse::Rejected,
    }
}
