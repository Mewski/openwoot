#[cfg(target_os = "macos")]
pub(super) const WITHOUT_XBOX_DRIVER: Option<&str> = Some("Mac");

#[cfg(not(target_os = "macos"))]
pub(super) const WITHOUT_XBOX_DRIVER: Option<&str> = None;
