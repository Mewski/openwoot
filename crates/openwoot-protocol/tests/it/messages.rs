use openwoot_protocol::{
    Body, ERROR_KEY, Endpoint, Error, FirmwareVersion, PROTOCOL_VERSION, VarHeader, VarKey, VarSeq,
    Version, WireError, decode_message, decode_response, encode_frame, encode_message,
    encode_request,
    endpoints::{EnterUpdateMode, GetVersion},
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

#[test]
fn a_reply_with_bytes_behind_its_body_is_not_decoded() {
    let header = VarHeader {
        key: VarKey::Key8(GetVersion::RESP_KEY),
        seq_no: VarSeq::Seq2(7),
    };
    let mut response = [0; MAX_LEN];
    let response = encode_message(&header, &(VERSION, 0xEE_u8), &mut response).unwrap();
    assert_eq!(
        decode_response::<GetVersion>(7, response),
        Err(Error::Decode)
    );
}

#[test]
fn a_reply_that_does_not_hold_the_response_is_not_decoded() {
    let header = VarHeader {
        key: VarKey::Key8(GetVersion::RESP_KEY),
        seq_no: VarSeq::Seq2(7),
    };
    let mut response = [0; MAX_LEN];
    let response = encode_message(&header, &(), &mut response).unwrap();
    assert_eq!(
        decode_response::<GetVersion>(7, response),
        Err(Error::Decode)
    );
}

#[test]
fn a_reply_of_another_endpoint_is_not_taken() {
    let header = VarHeader {
        key: VarKey::Key8(EnterUpdateMode::RESP_KEY),
        seq_no: VarSeq::Seq2(7),
    };
    let mut response = [0; MAX_LEN];
    let response = encode_message(&header, &(), &mut response).unwrap();
    assert_eq!(decode_response::<GetVersion>(7, response), Ok(None));
}

#[test]
fn a_reply_with_another_key_or_sequence_number_size_is_not_taken() {
    let mut response = [0; MAX_LEN];
    for seq_no in [VarSeq::Seq1(7), VarSeq::Seq4(7)] {
        let header = VarHeader {
            key: VarKey::Key8(GetVersion::RESP_KEY),
            seq_no,
        };
        let response = encode_message(&header, &VERSION, &mut response).unwrap();
        assert_eq!(decode_response::<GetVersion>(7, response), Ok(None));
    }
    let request = [1, b'O', b'W', 3, 0, 0x00, 0x55, 7];
    let message = decode_message(&request).unwrap().unwrap();
    let header = message.reply_header(GetVersion::RESP_KEY);
    assert!(matches!(header.key, VarKey::Key1(_)));
    let response = encode_message(&header, &VERSION, &mut response).unwrap();
    assert_eq!(decode_response::<GetVersion>(7, response), Ok(None));
}

#[test]
fn a_body_behind_its_trait_object_frames_the_same_report_as_its_type() {
    let header = VarHeader {
        key: VarKey::Key8(GetVersion::RESP_KEY),
        seq_no: VarSeq::Seq2(7),
    };
    let mut typed = [0; MAX_LEN];
    let typed = encode_message(&header, &VERSION, &mut typed).unwrap();
    let body: &dyn Body = &VERSION;
    let mut erased = [0; MAX_LEN];
    assert_eq!(encode_frame(&header, body, &mut erased), Ok(typed));
}

#[test]
fn a_body_beyond_the_report_is_too_long_for_the_frame() {
    let header = VarHeader {
        key: VarKey::Key8(GetVersion::RESP_KEY),
        seq_no: VarSeq::Seq2(7),
    };
    let mut smallest = [0; 1 + SIZES[0]];
    assert_eq!(
        encode_frame(&header, &[0xA5_u8; 32], &mut smallest),
        Err(Error::FrameTooLong)
    );
    let mut largest = [0; MAX_LEN];
    assert_eq!(
        encode_message(&header, &[0xA5_u8; MAX_LEN][..], &mut largest),
        Err(Error::FrameTooLong)
    );
    let mut buffer = [0; MAX_LEN];
    let length = VERSION.encode(&mut buffer).unwrap();
    assert_eq!(
        VERSION.encode(&mut buffer[..length - 1]),
        Err(Error::FrameTooLong)
    );
}
