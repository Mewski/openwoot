use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hex(pub u32);

impl fmt::Display for Hex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "0x{:04x}_{:04x}", self.0 >> 16, self.0 & 0xFFFF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_groups_four_digits() {
        assert_eq!(Hex(0x0800_8000).to_string(), "0x0800_8000");
        assert_eq!(Hex(0xDC_BEA4).to_string(), "0x00dc_bea4");
    }
}
