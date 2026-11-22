use openwoot_protocol::{
    Error, decode_message, decode_response,
    endpoints::GetVersion,
    report::{self, Contents, FRAME_OFFSET, MAX_FRAME_LEN, MAX_LEN, SIZES},
};

#[test]
fn each_frame_goes_into_the_smallest_report_that_holds_it() {
    let data_overhead = FRAME_OFFSET - 1;
    for (report_id, size) in (1..).zip(SIZES) {
        for frame_len in [size - data_overhead, size - data_overhead - 1] {
            let mut report = [0xAA; MAX_LEN];
            let sealed = report::seal(&mut report, frame_len).unwrap();
            assert_eq!(sealed.len(), 1 + size);
            assert_eq!(sealed[0], report_id);
            assert!(
                sealed[FRAME_OFFSET + frame_len..]
                    .iter()
                    .all(|&byte| byte == 0)
            );
            assert_eq!(
                report::parse(sealed).unwrap(),
                Contents::Frame(&sealed[FRAME_OFFSET..][..frame_len])
            );
        }
    }
    let mut report = [0; MAX_LEN];
    assert_eq!(
        report::seal(&mut report, MAX_FRAME_LEN).map(<[u8]>::len),
        Ok(MAX_LEN)
    );
    assert_eq!(
        report::seal(&mut report, MAX_FRAME_LEN + 1),
        Err(Error::FrameTooLong)
    );
}

#[test]
fn stock_traffic_is_foreign() {
    let stock = [1, 0xD1, 0xDA, 0x01, 0x00];
    assert_eq!(
        report::parse(&stock),
        Ok(Contents::Foreign {
            report_id: 1,
            payload: &stock[1..]
        })
    );
    assert_eq!(decode_message(&stock), Ok(None));
    assert_eq!(decode_response::<GetVersion>(0, &stock), Ok(None));
}

#[test]
fn a_frame_longer_than_its_report_is_malformed() {
    let report = [1, b'O', b'W', 0xFF, 0x00, 0x00];
    assert_eq!(report::parse(&report), Err(Error::MalformedReport));
}
