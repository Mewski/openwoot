use openwoot_protocol::report;

use super::{
    descriptor::{Builder, Collection, MainFlags},
    usage::page,
};

pub const DESCRIPTOR: &[u8] = &descriptor().to_array::<{ descriptor().len() }>();
pub(super) const FEATURE_REPORT_ID: u8 = 1;
pub const FEATURE_REPORT_LEN: usize = 7;

const USAGE_STEP: u16 = 10;
const FIRST_INPUT_USAGE: u16 = 0x0C;
const FEATURE_USAGE: u16 = 0x10;

const fn descriptor() -> Builder {
    let mut builder = Builder::new()
        .usage_page(page::VENDOR_CHANNEL)
        .usage(0x01)
        .collection(Collection::Application);
    let mut sizes: &[usize] = &report::SIZES;
    let mut report_id = 1;
    while let [size, rest @ ..] = sizes {
        let input_usage = FIRST_INPUT_USAGE + (report_id as u16 - 1) * USAGE_STEP;
        builder = builder.report_id(report_id).usage(input_usage);
        if report_id == 1 {
            builder = builder
                .logical_minimum(0)
                .logical_maximum(0xFF)
                .report_size(8);
        }
        builder = builder
            .report_count(*size)
            .input(MainFlags::DATA.variable())
            .usage(input_usage + 2)
            .output(MainFlags::DATA.variable());
        if report_id == FEATURE_REPORT_ID {
            builder = builder
                .usage(FEATURE_USAGE)
                .report_count(FEATURE_REPORT_LEN)
                .feature(MainFlags::DATA.variable());
        }
        report_id += 1;
        sizes = rest;
    }
    builder.end_collection()
}
