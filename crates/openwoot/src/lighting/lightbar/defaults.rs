use heapless::Vec;
use openwoot_protocol::lightbar::{
    Background, Effect, Entries, Entry, ProfileIndicator, Range, Settings, Title,
};

#[must_use]
pub const fn lightbar_profile(brightness: Option<u8>) -> Settings {
    Settings {
        enabled: true,
        entries: Vec::new(),
        use_global: true,
        brightness,
    }
}

#[must_use]
pub const fn lightbar_indicator(leds: u8, background: Background, segments: u8) -> Entries {
    Vec::from_array([Entry {
        title: Title::new(),
        condition: None,
        effect: Effect::ProfileIndicator(ProfileIndicator {
            range: Range {
                first: 0,
                last: leds.saturating_sub(1),
            },
            background,
            segments,
        }),
    }])
}
