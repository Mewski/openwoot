use embassy_usb::{
    Handler,
    control::{InResponse, OutResponse, Recipient, Request, RequestType},
    types::StringIndex,
};

use super::{
    HidProtocol, HostState, hid_class,
    interfaces::{CHANNEL_INTERFACE, INTERFACE_COUNT, KEYBOARD_INTERFACE},
};
use crate::xinput;

const GET_REPORT: u8 = 0x01;
const GET_IDLE: u8 = 0x02;
const GET_PROTOCOL: u8 = 0x03;
const SET_REPORT: u8 = 0x09;
const SET_IDLE: u8 = 0x0A;
const SET_PROTOCOL: u8 = 0x0B;
const OUTPUT_REPORT_TYPE: u8 = 0x02;
const FEATURE_REPORT_TYPE: u8 = 0x03;

pub(super) struct Control<'d> {
    host: &'d HostState,
    interfaces: [Option<HidInterface>; INTERFACE_COUNT],
    xinput: bool,
}

#[derive(Debug, Clone, Copy)]
struct HidInterface {
    report_descriptor: &'static [u8],
    idle: u8,
    protocol: HidProtocol,
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
                protocol: HidProtocol::Report,
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

    fn receive_report(
        &self,
        interface: usize,
        [report_type, report_id]: [u8; 2],
        data: &[u8],
    ) -> OutResponse {
        match (interface, report_type, data) {
            (KEYBOARD_INTERFACE, OUTPUT_REPORT_TYPE, [leds, ..]) => {
                self.host.set_keyboard_leds(*leds);
                OutResponse::Accepted
            }
            (CHANNEL_INTERFACE, FEATURE_REPORT_TYPE, [first, rest @ ..])
                if report_id != 0 && *first == report_id =>
            {
                self.host.feature_requests().submit(rest);
                OutResponse::Accepted
            }
            (CHANNEL_INTERFACE, FEATURE_REPORT_TYPE, _) => {
                self.host.feature_requests().submit(data);
                OutResponse::Accepted
            }
            (CHANNEL_INTERFACE, ..) => OutResponse::Accepted,
            _ => OutResponse::Rejected,
        }
    }
}

impl Handler for Control<'_> {
    fn reset(&mut self) {
        self.host.count_reset();
        for interface in self.interfaces.iter_mut().flatten() {
            interface.idle = 0;
            interface.protocol = HidProtocol::Report;
        }
        self.host.set_keyboard_protocol(HidProtocol::Report);
        self.host.set_suspended(false);
        self.host.set_configured(false);
    }

    fn enabled(&mut self, enabled: bool) {
        if !enabled {
            self.host.set_configured(false);
        }
    }

    fn configured(&mut self, configured: bool) {
        self.host.set_configured(configured);
    }

    fn suspended(&mut self, suspended: bool) {
        self.host.set_suspended(suspended);
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
                    HidProtocol::Boot
                } else {
                    HidProtocol::Report
                };
                interface.protocol = protocol;
                if number == KEYBOARD_INTERFACE {
                    host.set_keyboard_protocol(protocol);
                }
                Some(OutResponse::Accepted)
            }
            SET_REPORT => Some(self.receive_report(number, [high, low], data)),
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
        let [high, low] = req.value.to_be_bytes();
        let (number, interface) = self.hid_interface(&req)?;
        let response = match (req.request_type, req.request) {
            (RequestType::Standard, Request::GET_DESCRIPTOR) => match high {
                hid_class::REPORT_DESCRIPTOR_TYPE => {
                    InResponse::Accepted(interface.report_descriptor)
                }
                hid_class::DESCRIPTOR_TYPE => {
                    copy_into(buf, &hid_class::descriptor(interface.report_descriptor))
                }
                _ => InResponse::Rejected,
            },
            (RequestType::Class, GET_REPORT)
                if number == CHANNEL_INTERFACE && low != 0 && req.length > 1 =>
            {
                copy_into(buf, &[low])
            }
            (RequestType::Class, GET_IDLE) => copy_into(buf, &[interface.idle]),
            (RequestType::Class, GET_PROTOCOL) => {
                copy_into(buf, &[u8::from(interface.protocol == HidProtocol::Report)])
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

fn copy_into<'a>(buf: &'a mut [u8], bytes: &[u8]) -> InResponse<'a> {
    match buf.get_mut(..bytes.len()) {
        Some(response) => {
            response.copy_from_slice(bytes);
            InResponse::Accepted(response)
        }
        None => InResponse::Rejected,
    }
}
