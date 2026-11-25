use objc2_core_graphics::{
    CGColorSpace, CGDisplayCopyColorSpace, CGPreflightScreenCaptureAccess,
    CGRequestScreenCaptureAccess,
};

const SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

pub(crate) fn denied() -> Option<&'static str> {
    if CGPreflightScreenCaptureAccess() {
        return None;
    }
    CGRequestScreenCaptureAccess();
    Some(SETTINGS)
}

pub(crate) fn profile(display: u32) -> Option<Vec<u8>> {
    CGColorSpace::icc_data(Some(&CGDisplayCopyColorSpace(display))).map(|icc| icc.to_vec())
}
