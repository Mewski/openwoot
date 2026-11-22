use openwoot_drivers::spi_nor::{Identification, UNIQUE_ID_LEN};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NorIdentity {
    pub identification: Identification,
    pub unique_id: [u8; UNIQUE_ID_LEN],
}
