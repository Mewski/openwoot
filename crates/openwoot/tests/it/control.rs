use std::pin::pin;

use openwoot::usb::{Face, HidProtocol, HostState, Resources, build};

use crate::support::{Host, IDENTITY, Outcome, setup};

const CLASS_INTERFACE_OUT: u8 = 0x21;
const CLASS_INTERFACE_IN: u8 = 0xA1;
const GET_IDLE: u8 = 0x02;
const GET_PROTOCOL: u8 = 0x03;
const SET_REPORT: u8 = 0x09;
const SET_IDLE: u8 = 0x0A;
const SET_PROTOCOL: u8 = 0x0B;

#[test]
fn hid_class_requests_reach_the_host_state() {
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let mut device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut run = pin!(device.usb.run());
    host.attach(run.as_mut());

    assert_eq!(state.keyboard_protocol(), HidProtocol::Report);
    assert_eq!(
        host.control_out(
            run.as_mut(),
            setup(CLASS_INTERFACE_OUT, SET_PROTOCOL, 0, 1, 0),
            &[]
        ),
        Some(Outcome::Accepted)
    );
    assert_eq!(state.keyboard_protocol(), HidProtocol::Boot);
    assert_eq!(
        host.control_in(
            run.as_mut(),
            setup(CLASS_INTERFACE_IN, GET_PROTOCOL, 0, 1, 1)
        ),
        Ok(vec![0])
    );

    assert_eq!(
        host.control_out(
            run.as_mut(),
            setup(CLASS_INTERFACE_OUT, SET_REPORT, 0x0200, 1, 1),
            &[0x02]
        ),
        Some(Outcome::Accepted)
    );
    assert!(state.keyboard_leds().caps_lock());
    assert_eq!(
        host.control_out(
            run.as_mut(),
            setup(CLASS_INTERFACE_OUT, SET_REPORT, 0x0301, 2, 8),
            &[1, 0, 0, 0, 0, 0, 0, 0]
        ),
        Some(Outcome::Accepted)
    );
    assert_eq!(
        host.control_out(
            run.as_mut(),
            setup(CLASS_INTERFACE_OUT, SET_REPORT, 0x0301, 3, 8),
            &[1, 0, 0, 0, 0, 0, 0, 0]
        ),
        Some(Outcome::Rejected)
    );

    assert_eq!(
        host.control_out(
            run.as_mut(),
            setup(CLASS_INTERFACE_OUT, SET_IDLE, 0x7D00, 3, 0),
            &[]
        ),
        Some(Outcome::Accepted)
    );
    assert_eq!(
        host.control_in(run.as_mut(), setup(CLASS_INTERFACE_IN, GET_IDLE, 0, 3, 1)),
        Ok(vec![0x7D])
    );
    assert_eq!(
        host.control_in(run.as_mut(), setup(0x81, 0x06, 0x2100, 4, 9)),
        Ok(vec![9, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 21, 0])
    );
}
