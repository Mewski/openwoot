#[macro_export]
macro_rules! app_descriptor {
    ($descriptor:expr) => {
        // SAFETY: `.app_descriptor` is placed behind the vector table by the board's `memory.x`
        // and holds only this static, whose type has a fixed `repr(C)` layout.
        #[unsafe(link_section = ".app_descriptor")]
        #[used]
        static APP_DESCRIPTOR: $crate::openwoot_image::Descriptor = $descriptor;
    };
}
