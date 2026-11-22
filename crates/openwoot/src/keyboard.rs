use openwoot_image::Descriptor;

pub trait Keyboard: 'static {
    const DESCRIPTOR: Descriptor;
    const KEY_ROWS: usize;
    const KEY_COLUMNS: usize;

    fn enter_update_mode(&mut self, option: u8);
}
