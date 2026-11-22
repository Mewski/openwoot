use openwoot_image::Descriptor;

pub trait Keyboard: 'static {
    const DESCRIPTOR: Descriptor;
    const KEY_ROWS: usize;
    const KEY_COLUMNS: usize;
}
