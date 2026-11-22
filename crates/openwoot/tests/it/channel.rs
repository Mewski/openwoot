use std::pin::pin;

use openwoot::{
    protocol::{Channel, Handler},
    usb::{Face, HostState, Resources, build},
};
use openwoot_protocol::{
    Error, FirmwareVersion, GetVersion, Key, MAX_REPORT_LEN, PROTOCOL_VERSION, VarHeader, VarKey,
    VarSeq, Version, WireError, decode_response, encode_message, encode_request,
};

use crate::support::{Host, IDENTITY, poll};

const CHANNEL_OUT: u8 = 0x03;
const CHANNEL_IN: u8 = 0x83;
const PACKET_SIZE: usize = 64;
const VERSION: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 1,
        minor: 2,
        patch: 3,
    },
};

fn exchange(request: &[u8]) -> Vec<u8> {
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut channel = Channel::new(device.channel.output, device.channel.input);
    let mut handler = Handler::new(VERSION);
    host.send_packets(CHANNEL_OUT, request, PACKET_SIZE);
    let _ = poll(pin!(channel.serve(&mut handler)));
    host.take_sent(CHANNEL_IN).concat()
}

#[test]
fn get_version_round_trips_through_the_channel_endpoints() {
    let mut request = [0; MAX_REPORT_LEN];
    let request = encode_request::<GetVersion>(42, &(), &mut request).unwrap();
    let response = exchange(request);
    assert_eq!(response.len(), 33);
    assert_eq!(response[..3], [1, b'O', b'W']);
    assert_eq!(
        decode_response::<GetVersion>(42, &response),
        Ok(Some(VERSION))
    );
}

#[test]
fn an_unknown_endpoint_gets_an_error_reply() {
    let header = VarHeader {
        key: VarKey::Key8(Key::for_path::<u8>("openwoot/unknown")),
        seq_no: VarSeq::Seq2(9),
    };
    let mut request = [0; MAX_REPORT_LEN];
    let request = encode_message(&header, &7_u8, &mut request).unwrap();
    let response = exchange(request);
    assert_eq!(
        decode_response::<GetVersion>(9, &response),
        Err(Error::Device(WireError::UnknownKey))
    );
}

#[test]
fn a_large_request_spans_many_packets() {
    let header = VarHeader {
        key: VarKey::Key8(Key::for_path::<u8>("openwoot/bulk")),
        seq_no: VarSeq::Seq2(1),
    };
    let mut request = [0; MAX_REPORT_LEN];
    let body = [0xA5_u8; 1500];
    let request = encode_message(&header, &body[..], &mut request).unwrap();
    assert_eq!(request.len(), 2047);
    let response = exchange(request);
    assert_eq!(
        decode_response::<GetVersion>(1, &response),
        Err(Error::Device(WireError::UnknownKey))
    );
}

#[test]
fn stock_traffic_gets_no_answer() {
    let mut request = [0_u8; 33];
    request[..4].copy_from_slice(&[1, 0xD1, 0xDA, 0x01]);
    assert_eq!(exchange(&request), Vec::<u8>::new());
}
