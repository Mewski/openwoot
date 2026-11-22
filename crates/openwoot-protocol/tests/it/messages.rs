use openwoot_protocol::{
    ERROR_KEY, Endpoint, Error, FirmwareVersion, PROTOCOL_VERSION, VarKey, VarSeq, Version,
    WireError, decode_message, decode_response, encode_message, encode_request,
    endpoints::GetVersion,
    report::{MAGIC, MAX_LEN, SIZES},
};

const VERSION: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 0,
        minor: 1,
        patch: 2,
    },
};

#[test]
fn get_version_survives_a_round_trip_through_the_reports() {
    let mut request = [0; MAX_LEN];
    let request = encode_request::<GetVersion>(7, &(), &mut request).unwrap();
    assert_eq!(request.len(), 1 + SIZES[0]);
    assert_eq!(request[0], 1);
    assert_eq!(request[1..3], MAGIC);

    let mut response = [0; MAX_LEN];
    let message = decode_message(request).unwrap().unwrap();
    assert_eq!(message.header.key, VarKey::Key8(GetVersion::REQ_KEY));
    let header = message.reply_header(GetVersion::RESP_KEY);
    let len = encode_message(&header, &VERSION, &mut response)
        .unwrap()
        .len();
    assert_eq!(
        decode_response::<GetVersion>(7, &response[..len]),
        Ok(Some(VERSION))
    );
    assert_eq!(decode_response::<GetVersion>(8, &response[..len]), Ok(None));
}

#[test]
fn a_device_error_reaches_the_host() {
    let mut request = [0; MAX_LEN];
    let request = encode_request::<GetVersion>(3, &(), &mut request).unwrap();
    let message = decode_message(request).unwrap().unwrap();
    let mut response = [0; MAX_LEN];
    let response = encode_message(
        &message.reply_header(ERROR_KEY),
        &WireError::UnknownKey,
        &mut response,
    )
    .unwrap();
    assert_eq!(
        decode_response::<GetVersion>(3, response),
        Err(Error::Device(WireError::UnknownKey))
    );
}

#[test]
fn reply_keeps_the_key_size_and_sequence_number_of_the_request() {
    let mut request = [0; MAX_LEN];
    let request = encode_request::<GetVersion>(0x1234, &(), &mut request).unwrap();
    let message = decode_message(request).unwrap().unwrap();
    let header = message.reply_header(GetVersion::RESP_KEY);
    assert!(matches!(header.key, VarKey::Key8(_)));
    assert_eq!(header.seq_no, VarSeq::Seq2(0x1234));
}
