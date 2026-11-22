use openwoot_image::Descriptor;

use crate::scan::Sensing;

pub trait Keyboard: 'static {
    type Sensing: Sensing;

    const DESCRIPTOR: Descriptor;
    const KEY_ROWS: usize;
    const KEY_COLUMNS: usize;

    fn enter_update_mode(&mut self, option: u8);
}
