use super::{
    descriptor::{Builder, Collection, MainFlags},
    usage::page,
};

pub const DESCRIPTOR: &[u8] = &descriptor().to_array::<{ descriptor().len() }>();
pub const LEN: usize = 8;

const fn descriptor() -> Builder {
    Builder::new()
        .usage_page(page::VENDOR_PLACEHOLDER)
        .usage(0x01)
        .collection(Collection::Application)
        .usage(0x02)
        .logical_minimum(0)
        .logical_maximum(0xFF)
        .report_size(8)
        .report_count(LEN)
        .input(MainFlags::DATA.variable())
        .end_collection()
}
