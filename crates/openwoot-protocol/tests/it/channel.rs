use openwoot_protocol::{
    Contents, ERROR_KEY, Endpoint, Error, FRAME_OFFSET, FirmwareVersion, GetVersion, MAGIC,
    MAX_FRAME_LEN, MAX_REPORT_LEN, PROTOCOL_VERSION, REPORT_SIZES, VarKey, VarSeq, Version,
    WireError, decode_message, decode_response, encode_message, encode_request, parse_report,
    seal_report,
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
    let mut request = [0; MAX_REPORT_LEN];
    let request = encode_request::<GetVersion>(7, &(), &mut request).unwrap();
    assert_eq!(request.len(), 1 + REPORT_SIZES[0]);
    assert_eq!(request[0], 1);
    assert_eq!(request[1..3], MAGIC);

    let mut response = [0; MAX_REPORT_LEN];
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
    let mut request = [0; MAX_REPORT_LEN];
    let request = encode_request::<GetVersion>(3, &(), &mut request).unwrap();
    let message = decode_message(request).unwrap().unwrap();
    let mut response = [0; MAX_REPORT_LEN];
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
    let mut request = [0; MAX_REPORT_LEN];
    let request = encode_request::<GetVersion>(0x1234, &(), &mut request).unwrap();
    let message = decode_message(request).unwrap().unwrap();
    let header = message.reply_header(GetVersion::RESP_KEY);
    assert!(matches!(header.key, VarKey::Key8(_)));
    assert_eq!(header.seq_no, VarSeq::Seq2(0x1234));
}

#[test]
fn each_frame_goes_into_the_smallest_report_that_holds_it() {
    let data_overhead = FRAME_OFFSET - 1;
    for (report_id, size) in (1..).zip(REPORT_SIZES) {
        for frame_len in [size - data_overhead, size - data_overhead - 1] {
            let mut report = [0xAA; MAX_REPORT_LEN];
            let sealed = seal_report(&mut report, frame_len).unwrap();
            assert_eq!(sealed.len(), 1 + size);
            assert_eq!(sealed[0], report_id);
            assert!(
                sealed[FRAME_OFFSET + frame_len..]
                    .iter()
                    .all(|&byte| byte == 0)
            );
            assert_eq!(
                parse_report(sealed).unwrap(),
                Contents::Frame(&sealed[FRAME_OFFSET..][..frame_len])
            );
        }
    }
    let mut report = [0; MAX_REPORT_LEN];
    assert_eq!(
        seal_report(&mut report, MAX_FRAME_LEN).map(<[u8]>::len),
        Ok(MAX_REPORT_LEN)
    );
    assert_eq!(
        seal_report(&mut report, MAX_FRAME_LEN + 1),
        Err(Error::FrameTooLong)
    );
}

#[test]
fn stock_traffic_is_foreign() {
    let stock = [1, 0xD1, 0xDA, 0x01, 0x00];
    assert_eq!(
        parse_report(&stock),
        Ok(Contents::Foreign {
            report_id: 1,
            data: &stock[1..]
        })
    );
    assert_eq!(decode_message(&stock), Ok(None));
    assert_eq!(decode_response::<GetVersion>(0, &stock), Ok(None));
}

#[test]
fn a_frame_longer_than_its_report_is_malformed() {
    let report = [1, b'O', b'W', 0xFF, 0x00, 0x00];
    assert_eq!(parse_report(&report), Err(Error::MalformedReport));
}
