use postcard_rpc::endpoints;

use crate::{
    Version, advanced,
    config::{Capabilities, GlobalSettings, Rejection},
    device::{self, Layout},
    gamepad, keys, lightbar, lighting,
    profile::{self, Change, LayerChange, LayerRef, Slot, Summary},
    switches,
};

type Done = Result<(), Rejection>;
type CapabilitiesReply = Result<Capabilities, Rejection>;
type DeviceSettingsReply = Result<device::Settings, Rejection>;
type LayoutReply = Result<Layout, Rejection>;
type GlobalSettingsReply = Result<GlobalSettings, Rejection>;
type SummaryReply = Result<Summary, Rejection>;
type SlotReply = Result<Slot, Rejection>;
type ProfileInfoReply = Result<profile::Info, Rejection>;
type ProfileInfoChange = Change<profile::Info>;
type RevisionReply = Result<u32, Rejection>;
type KeySettingsReply = Result<keys::Settings, Rejection>;
type KeySettingsChange = Change<keys::Settings>;
type KeymapReply = Result<keys::Keymap, Rejection>;
type KeymapChange = LayerChange<keys::Keymap>;
type OverridesReply = Result<keys::Overrides, Rejection>;
type OverridesChange = Change<keys::Overrides>;
type AdvancedKeysReply = Result<advanced::Keys, Rejection>;
type AdvancedKeysChange = Change<advanced::Keys>;
type GamepadMappingReply = Result<gamepad::Mapping, Rejection>;
type GamepadMappingChange = Change<gamepad::Mapping>;
type GamepadSettingsReply = Result<gamepad::Settings, Rejection>;
type GamepadSettingsChange = Change<gamepad::Settings>;
type LightingReply = Result<lighting::Settings, Rejection>;
type LightingChange = Change<lighting::Settings>;
type KeyColorsReply = Result<Option<lighting::KeyColors>, Rejection>;
type KeyColorsChange = LayerChange<Option<lighting::KeyColors>>;
type LightbarReply = Result<lightbar::Settings, Rejection>;
type LightbarChange = Change<lightbar::Settings>;
type GlobalLightbarReply = Result<lightbar::Entries, Rejection>;
type HostSignalIdsReply = Result<lightbar::HostSignalIds, Rejection>;
type HostSignals = lightbar::HostSignals;
type SwitchesReply = Result<switches::Settings, Rejection>;
type SwitchSettings = switches::Settings;
type SwitchModelsReply = Result<switches::Models, Rejection>;
type DeviceSettings = device::Settings;
type GlobalEntries = lightbar::Entries;

endpoints! {
    list = ENDPOINT_LIST;
    | EndpointTy            | RequestTy            | ResponseTy           | Path                                |
    | ----------            | ---------            | ----------           | ----                                |
    | GetVersion            | ()                   | Version              | "openwoot/version"                  |
    | EnterUpdateMode       | ()                   | ()                   | "openwoot/update-mode"              |
    | GetCapabilities       | ()                   | CapabilitiesReply    | "openwoot/capabilities"             |
    | GetDeviceSettings     | ()                   | DeviceSettingsReply  | "openwoot/device"                   |
    | SetDeviceSettings     | DeviceSettings       | Done                 | "openwoot/device/set"               |
    | GetLayout             | ()                   | LayoutReply          | "openwoot/layout"                   |
    | SetLayout             | Layout               | Done                 | "openwoot/layout/set"               |
    | GetGlobalSettings     | ()                   | GlobalSettingsReply  | "openwoot/global"                   |
    | SetGlobalSettings     | GlobalSettings       | Done                 | "openwoot/global/set"               |
    | GetProfiles           | ()                   | SummaryReply         | "openwoot/profiles"                 |
    | CreateProfile         | ()                   | SlotReply            | "openwoot/profiles/create"          |
    | DeleteProfile         | Slot                 | Done                 | "openwoot/profiles/delete"          |
    | ActivateProfile       | Slot                 | Done                 | "openwoot/profiles/activate"        |
    | GetProfileInfo        | Slot                 | ProfileInfoReply     | "openwoot/profile/info"             |
    | SetProfileInfo        | ProfileInfoChange    | Done                 | "openwoot/profile/info/set"         |
    | GetProfileRevision    | Slot                 | RevisionReply        | "openwoot/profile/revision"         |
    | GetKeySettings        | Slot                 | KeySettingsReply     | "openwoot/profile/keys"             |
    | SetKeySettings        | KeySettingsChange    | Done                 | "openwoot/profile/keys/set"         |
    | GetKeymap             | LayerRef             | KeymapReply          | "openwoot/profile/keymap"           |
    | SetKeymap             | KeymapChange         | Done                 | "openwoot/profile/keymap/set"       |
    | GetKeyOverrides       | Slot                 | OverridesReply       | "openwoot/profile/overrides"        |
    | SetKeyOverrides       | OverridesChange      | Done                 | "openwoot/profile/overrides/set"    |
    | GetAdvancedKeys       | Slot                 | AdvancedKeysReply    | "openwoot/profile/advanced"         |
    | SetAdvancedKeys       | AdvancedKeysChange   | Done                 | "openwoot/profile/advanced/set"     |
    | GetGamepadMapping     | Slot                 | GamepadMappingReply  | "openwoot/profile/gamepad/mapping"  |
    | SetGamepadMapping     | GamepadMappingChange | Done                 | "openwoot/profile/gamepad/mapping/set" |
    | GetGamepadSettings    | Slot                 | GamepadSettingsReply | "openwoot/profile/gamepad"          |
    | SetGamepadSettings    | GamepadSettingsChange | Done                | "openwoot/profile/gamepad/set"      |
    | GetLighting           | Slot                 | LightingReply        | "openwoot/profile/lighting"         |
    | SetLighting           | LightingChange       | Done                 | "openwoot/profile/lighting/set"     |
    | GetKeyColors          | LayerRef             | KeyColorsReply       | "openwoot/profile/colors"           |
    | SetKeyColors          | KeyColorsChange      | Done                 | "openwoot/profile/colors/set"       |
    | GetLightbar           | Slot                 | LightbarReply        | "openwoot/profile/lightbar"         |
    | SetLightbar           | LightbarChange       | Done                 | "openwoot/profile/lightbar/set"     |
    | GetGlobalLightbar     | ()                   | GlobalLightbarReply  | "openwoot/lightbar"                 |
    | SetGlobalLightbar     | GlobalEntries        | Done                 | "openwoot/lightbar/set"             |
    | GetHostSignals        | ()                   | HostSignalIdsReply   | "openwoot/signals"                  |
    | SetHostSignals        | HostSignals          | Done                 | "openwoot/signals/set"              |
    | GetSwitches           | ()                   | SwitchesReply        | "openwoot/switches"                 |
    | SetSwitches           | SwitchSettings       | Done                 | "openwoot/switches/set"             |
    | GetSwitchModels       | ()                   | SwitchModelsReply    | "openwoot/switches/models"          |
    | Save                  | ()                   | Done                 | "openwoot/save"                     |
    | Revert                | ()                   | Done                 | "openwoot/revert"                   |
    | FactoryReset          | ()                   | Done                 | "openwoot/factory-reset"            |
}
