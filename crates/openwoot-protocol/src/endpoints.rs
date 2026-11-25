use postcard_rpc::endpoints;

use crate::{
    Version, advanced,
    configuration::{Capabilities, Rejection},
    device::{self, Layout, LayoutState},
    diagnostics, gamepad, geometry, global, host_lighting, keys, lightbar, lighting,
    profile::{self, Change, LayerChange, LayerRef, Slot, Summary},
    sensors, switches,
};

type Done = Result<(), Rejection>;
type DiagnosticsReport = diagnostics::Report;
type CapabilitiesReply = Result<Capabilities, Rejection>;
type DeviceSettingsReply = Result<device::Settings, Rejection>;
type DeviceSettings = device::Settings;
type LayoutReply = Result<LayoutState, Rejection>;
type KeyGeometryReply = Result<geometry::Keys, Rejection>;
type KeymapLayer = keys::Layer;
type GlobalSettingsReply = Result<global::Settings, Rejection>;
type GlobalSettings = global::Settings;
type ProfilesReply = Result<Summary, Rejection>;
type CreatedProfileReply = Result<Slot, Rejection>;
type ProfileInfoReply = Result<profile::Info, Rejection>;
type ProfileSwap = profile::Swap;
type ProfileMove = profile::Move;
type ProfileInfoChange = Change<profile::Info>;
type ProfileRevisionReply = Result<u32, Rejection>;
type KeySettingsReply = Result<keys::Settings, Rejection>;
type KeySettingsChange = Change<keys::Settings>;
type KeymapReply = Result<keys::Keymap, Rejection>;
type KeymapChange = LayerChange<keys::Keymap>;
type KeyOverridesReply = Result<keys::Overrides, Rejection>;
type KeyOverridesChange = Change<keys::Overrides>;
type AdvancedKeysReply = Result<advanced::Keys, Rejection>;
type AdvancedKeysChange = Change<advanced::Keys>;
type GamepadMappingReply = Result<gamepad::Mapping, Rejection>;
type GamepadMappingChange = Change<gamepad::Mapping>;
type GamepadSettingsReply = Result<gamepad::Settings, Rejection>;
type GamepadSettingsChange = Change<gamepad::Settings>;
type GamepadStateReply = Result<gamepad::State, Rejection>;
type LightingReply = Result<lighting::Settings, Rejection>;
type LightingChange = Change<lighting::Settings>;
type KeyColorsReply = Result<Option<lighting::KeyColors>, Rejection>;
type KeyColorsChange = LayerChange<Option<lighting::KeyColors>>;
type LightbarReply = Result<lightbar::Settings, Rejection>;
type LightbarChange = Change<lightbar::Settings>;
type GlobalLightbarReply = Result<lightbar::Entries, Rejection>;
type GlobalLightbar = lightbar::Entries;
type HostSignalsReply = Result<lightbar::HostSignalIds, Rejection>;
type HostSignals = lightbar::HostSignals;
type SwitchesReply = Result<switches::Settings, Rejection>;
type Switches = switches::Settings;
type SwitchModelsReply = Result<switches::Models, Rejection>;
type SensorReading = sensors::Reading;
type SensorValuesReply = Result<sensors::Values, Rejection>;
type HostKeyColor = host_lighting::KeyColor;
type HostColors = lighting::KeyColors;
type KeyPosition = keys::Position;

endpoints! {
    list = LIST;
    | EndpointTy          | RequestTy             | ResponseTy           | Path                                   |
    | ----------          | ---------             | ----------           | ----                                   |
    | GetVersion          | ()                    | Version              | "openwoot/version"                     |
    | EnterUpdateMode     | ()                    | ()                   | "openwoot/update-mode"                 |
    | GetDiagnostics      | ()                    | DiagnosticsReport    | "openwoot/diagnostics"                 |
    | GetCapabilities     | ()                    | CapabilitiesReply    | "openwoot/capabilities"                |
    | GetDeviceSettings   | ()                    | DeviceSettingsReply  | "openwoot/device"                      |
    | SetDeviceSettings   | DeviceSettings        | Done                 | "openwoot/device/set"                  |
    | GetLayout           | ()                    | LayoutReply          | "openwoot/layout"                      |
    | SetLayout           | Layout                | Done                 | "openwoot/layout/set"                  |
    | GetKeyGeometry      | ()                    | KeyGeometryReply     | "openwoot/geometry"                    |
    | GetDefaultKeymap    | KeymapLayer           | KeymapReply          | "openwoot/default-keymap"              |
    | GetGlobalSettings   | ()                    | GlobalSettingsReply  | "openwoot/global"                      |
    | SetGlobalSettings   | GlobalSettings        | Done                 | "openwoot/global/set"                  |
    | GetProfiles         | ()                    | ProfilesReply        | "openwoot/profiles"                    |
    | CreateProfile       | ()                    | CreatedProfileReply  | "openwoot/profiles/create"             |
    | DeleteProfile       | Slot                  | Done                 | "openwoot/profiles/delete"             |
    | ActivateProfile     | Slot                  | Done                 | "openwoot/profiles/activate"           |
    | SwapProfiles        | ProfileSwap           | Done                 | "openwoot/profiles/swap"               |
    | MoveProfile         | ProfileMove           | Done                 | "openwoot/profiles/move"               |
    | GetProfileInfo      | Slot                  | ProfileInfoReply     | "openwoot/profile/info"                |
    | SetProfileInfo      | ProfileInfoChange     | Done                 | "openwoot/profile/info/set"            |
    | GetProfileRevision  | Slot                  | ProfileRevisionReply | "openwoot/profile/revision"            |
    | GetKeySettings      | Slot                  | KeySettingsReply     | "openwoot/profile/keys"                |
    | SetKeySettings      | KeySettingsChange     | Done                 | "openwoot/profile/keys/set"            |
    | GetKeymap           | LayerRef              | KeymapReply          | "openwoot/profile/keymap"              |
    | SetKeymap           | KeymapChange          | Done                 | "openwoot/profile/keymap/set"          |
    | GetKeyOverrides     | Slot                  | KeyOverridesReply    | "openwoot/profile/overrides"           |
    | SetKeyOverrides     | KeyOverridesChange    | Done                 | "openwoot/profile/overrides/set"       |
    | GetAdvancedKeys     | Slot                  | AdvancedKeysReply    | "openwoot/profile/advanced"            |
    | SetAdvancedKeys     | AdvancedKeysChange    | Done                 | "openwoot/profile/advanced/set"        |
    | GetGamepadMapping   | Slot                  | GamepadMappingReply  | "openwoot/profile/gamepad/mapping"     |
    | SetGamepadMapping   | GamepadMappingChange  | Done                 | "openwoot/profile/gamepad/mapping/set" |
    | GetGamepadSettings  | Slot                  | GamepadSettingsReply | "openwoot/profile/gamepad"             |
    | SetGamepadSettings  | GamepadSettingsChange | Done                 | "openwoot/profile/gamepad/set"         |
    | GetLighting         | Slot                  | LightingReply        | "openwoot/profile/lighting"            |
    | SetLighting         | LightingChange        | Done                 | "openwoot/profile/lighting/set"        |
    | GetKeyColors        | LayerRef              | KeyColorsReply       | "openwoot/profile/colors"              |
    | SetKeyColors        | KeyColorsChange       | Done                 | "openwoot/profile/colors/set"          |
    | GetLightbar         | Slot                  | LightbarReply        | "openwoot/profile/lightbar"            |
    | SetLightbar         | LightbarChange        | Done                 | "openwoot/profile/lightbar/set"        |
    | GetGlobalLightbar   | ()                    | GlobalLightbarReply  | "openwoot/lightbar"                    |
    | SetGlobalLightbar   | GlobalLightbar        | Done                 | "openwoot/lightbar/set"                |
    | GetHostSignals      | ()                    | HostSignalsReply     | "openwoot/signals"                     |
    | SetHostSignals      | HostSignals           | Done                 | "openwoot/signals/set"                 |
    | GetSwitches         | ()                    | SwitchesReply        | "openwoot/switches"                    |
    | SetSwitches         | Switches              | Done                 | "openwoot/switches/set"                |
    | GetSwitchModels     | ()                    | SwitchModelsReply    | "openwoot/switches/models"             |
    | SetKeyReports       | bool                  | Done                 | "openwoot/key-reports/set"             |
    | GetGamepadState     | ()                    | GamepadStateReply    | "openwoot/gamepad"                     |
    | SetGamepadDetection | bool                  | Done                 | "openwoot/gamepad-detection/set"       |
    | GetSensorValues     | SensorReading         | SensorValuesReply    | "openwoot/sensors"                     |
    | SetRawScanning      | bool                  | Done                 | "openwoot/sensors/raw-scanning/set"    |
    | SetHostKeyColor     | HostKeyColor          | Done                 | "openwoot/host-lighting/key/set"       |
    | SetHostColors       | HostColors            | Done                 | "openwoot/host-lighting/set"           |
    | ResetHostKeyColor   | KeyPosition           | Done                 | "openwoot/host-lighting/key/reset"     |
    | ResetHostColors     | ()                    | Done                 | "openwoot/host-lighting/reset"         |
    | LeaveHostControl    | ()                    | Done                 | "openwoot/host-lighting/leave"         |
    | Restart             | ()                    | ()                   | "openwoot/restart"                     |
    | Save                | ()                    | Done                 | "openwoot/save"                        |
    | Revert              | ()                    | Done                 | "openwoot/revert"                      |
    | FactoryReset        | ()                    | Done                 | "openwoot/factory-reset"               |
}
