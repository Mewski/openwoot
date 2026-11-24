use openwoot_protocol::{
    advanced,
    configuration::{Capabilities, Rejection},
    device::{self, Layout},
    gamepad, geometry, global,
    keys::{self, Layer},
    lightbar, lighting,
    profile::{self, Slot, Summary},
    switches,
};

#[expect(
    async_fn_in_trait,
    reason = "the configuration channel runs on one executor, so the futures need not be `Send`"
)]
pub trait Configuration {
    fn capabilities(&self) -> Result<Capabilities, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn device_settings(&mut self) -> Result<device::Settings, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_device_settings(&mut self, _settings: device::Settings) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn layout(&mut self) -> Result<Layout, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_layout(&mut self, _layout: Layout) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn key_geometry(&mut self, _keys: &mut geometry::Keys) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn global_settings(&mut self) -> Result<global::Settings, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_global_settings(&mut self, _settings: global::Settings) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn profiles(&mut self) -> Result<Summary, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn create_profile(&mut self) -> Result<Slot, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn delete_profile(&mut self, _profile: Slot) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn activate_profile(&mut self, _profile: Slot) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn profile_info(&mut self, _profile: Slot) -> Result<profile::Info, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_profile_info(
        &mut self,
        _profile: Slot,
        _info: profile::Info,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn profile_revision(&mut self, _profile: Slot) -> Result<u32, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn key_settings(&mut self, _profile: Slot) -> Result<keys::Settings, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_key_settings(
        &mut self,
        _profile: Slot,
        _settings: keys::Settings,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn keymap(
        &mut self,
        _profile: Slot,
        _layer: Layer,
        _keymap: &mut keys::Keymap,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_keymap(
        &mut self,
        _profile: Slot,
        _layer: Layer,
        _keymap: &keys::Keymap,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn key_overrides(
        &mut self,
        _profile: Slot,
        _overrides: &mut keys::Overrides,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_key_overrides(
        &mut self,
        _profile: Slot,
        _overrides: &keys::Overrides,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn advanced_keys(
        &mut self,
        _profile: Slot,
        _keys: &mut advanced::Keys,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_advanced_keys(
        &mut self,
        _profile: Slot,
        _keys: &advanced::Keys,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn gamepad_mapping(
        &mut self,
        _profile: Slot,
        _mapping: &mut gamepad::Mapping,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_gamepad_mapping(
        &mut self,
        _profile: Slot,
        _mapping: &gamepad::Mapping,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn gamepad_settings(&mut self, _profile: Slot) -> Result<gamepad::Settings, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_gamepad_settings(
        &mut self,
        _profile: Slot,
        _settings: gamepad::Settings,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn lighting(&mut self, _profile: Slot) -> Result<lighting::Settings, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_lighting(
        &mut self,
        _profile: Slot,
        _settings: lighting::Settings,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn key_colors(
        &mut self,
        _profile: Slot,
        _layer: Layer,
        _colors: &mut Option<lighting::KeyColors>,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_key_colors(
        &mut self,
        _profile: Slot,
        _layer: Layer,
        _colors: Option<&lighting::KeyColors>,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn lightbar(
        &mut self,
        _profile: Slot,
        _settings: &mut lightbar::Settings,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_lightbar(
        &mut self,
        _profile: Slot,
        _settings: &lightbar::Settings,
    ) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn global_lightbar(&mut self, _entries: &mut lightbar::Entries) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_global_lightbar(&mut self, _entries: &lightbar::Entries) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn host_signals(&mut self) -> Result<lightbar::HostSignalIds, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_host_signals(&mut self, _signals: lightbar::HostSignals) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn switches(&mut self) -> Result<switches::Settings, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn set_switches(&mut self, _settings: switches::Settings) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn switch_models(&mut self) -> Result<switches::Models, Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn save(&mut self) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn revert(&mut self) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }

    async fn factory_reset(&mut self) -> Result<(), Rejection> {
        Err(Rejection::Unsupported)
    }
}

impl Configuration for () {}
