use std::fmt::Write;

#[must_use]
pub(crate) fn to_hex(report: &[u8]) -> String {
    report.iter().fold(String::new(), |mut text, byte| {
        write!(text, "{byte:02x}").ok();
        text
    })
}
