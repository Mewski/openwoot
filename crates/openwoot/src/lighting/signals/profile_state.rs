use openwoot_protocol::{lighting::Rgb, profile::Slot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileState {
    pub slot: u8,
    pub count: u8,
    pub color: Rgb,
}

impl ProfileState {
    pub(crate) fn active_value(self) -> f32 {
        let profile = Slot::new(self.slot);
        let namespace = if profile.is_linked() { 256 } else { 0 };
        f32::from(namespace + u16::from(profile.index()) + 1)
    }

    pub(crate) fn count_value(self) -> f32 {
        f32::from(self.count)
    }

    pub(crate) const fn color_value(self) -> f32 {
        f32::from_bits(u32::from_be_bytes([
            0xFF,
            self.color.red,
            self.color.green,
            self.color.blue,
        ]))
    }
}

pub(crate) fn color_of(value: f32) -> Rgb {
    let [_, red, green, blue] = value.to_bits().to_be_bytes();
    Rgb { red, green, blue }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_colour_travels_as_the_bits_of_the_value() {
        let state = ProfileState {
            slot: 2,
            count: 3,
            color: Rgb {
                red: 0xFF,
                green: 0x28,
                blue: 0x10,
            },
        };
        assert_eq!(state.active_value(), 3.0);
        assert_eq!(state.count_value(), 3.0);
        assert_eq!(state.color_value().to_bits(), 0xFFFF_2810);
        assert!(state.color_value().is_nan());
        assert_eq!(color_of(state.color_value()), state.color);
    }
}
