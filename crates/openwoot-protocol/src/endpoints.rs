use postcard_rpc::endpoints;

use crate::{
    Version, advanced, apps,
    configuration::{Capabilities, Rejection},
    device::{self, Layout, LayoutState},
    diagnostics, gamepad, geometry, global, host_lighting, keys, lightbar, lighting, macros,
    profile::{self, Change, LayerChange, LayerRef, Slot, Summary},
    sensors, switches,
};

type Done = Result<(), Rejection>;
type DiagnosticsReport = diagnostics::Report;
type StorageUsageReply = Result<diagnostics::StorageUsage, Rejection>;
type CapabilitiesReply = Result<Capabilities, Rejection>;
type DeviceSettingsReply = Result<device::SettingsState, Rejection>;
type DeviceSettings = device::Settings;
type DeviceNameReply = Result<device::Name, Rejection>;
type DeviceName = device::Name;
type LayoutReply = Result<LayoutState, Rejection>;
type KeyGeometryReply = Result<geometry::Keys, Rejection>;
type KeymapLayer = keys::Layer;
type GlobalSettingsReply = Result<global::Settings, Rejection>;
type GlobalSettings = global::Settings;
type ProfilesReply = Result<Summary, Rejection>;
type ProfileNamespace = profile::Namespace;
type CreatedProfileReply = Result<Slot, Rejection>;
type ProfileSelection = profile::Selection;
type AppLinkCountReply = Result<u8, Rejection>;
type AppLinkRef = apps::LinkRef;
type AppLinkReply = Result<apps::Link, Rejection>;
type AppLinkChange = apps::LinkChange;
type ProfileInfoReply = Result<profile::Info, Rejection>;
type ProfileSwap = profile::Swap;
type ProfileMove = profile::Move;
type ProfileInfoChange = Change<profile::Info>;
type ProfileRevisionReply = Result<u32, Rejection>;
type KeySettingsReply = Result<keys::Settings, Rejection>;
type KeySettingsChange = Change<keys::Settings>;
type KeymapReply = Result<keys::Keymap, Rejection>;
type KeymapChange = LayerChange<keys::Keymap>;
type KeymapLinkReply = Result<bool, Rejection>;
type KeymapLinkChange = Change<bool>;
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
type HostLightbar = lightbar::Colors;
type LedFrameReply = Result<host_lighting::Frame, Rejection>;
type LightbarColorsReply = Result<lightbar::Colors, Rejection>;
type ProfileStateReply = Result<profile::State, Rejection>;
type MacrosReply = Result<macros::Summaries, Rejection>;
type MacroReply = Result<Option<macros::Macro>, Rejection>;
type MacroChange = macros::Change;

endpoints! {
    list = LIST;
    | EndpointTy          | RequestTy             | ResponseTy           | Path                                    |
    | ----------          | ---------             | ----------           | ----                                    |
    | GetVersion          | ()                    | Version              | "openwoot/version"                      |
    | EnterUpdateMode     | ()                    | ()                   | "openwoot/update-mode"                  |
    | GetDiagnostics      | ()                    | DiagnosticsReport    | "openwoot/diagnostics"                  |
    | GetStorageUsage     | ()                    | StorageUsageReply    | "openwoot/storage/usage"                |
    | GetCapabilities     | ()                    | CapabilitiesReply    | "openwoot/capabilities"                 |
    | GetDeviceSettings   | ()                    | DeviceSettingsReply  | "openwoot/device"                       |
    | SetDeviceSettings   | DeviceSettings        | Done                 | "openwoot/device/set"                   |
    | GetDeviceName       | ()                    | DeviceNameReply      | "openwoot/device/name"                  |
    | SetDeviceName       | DeviceName            | Done                 | "openwoot/device/name/set"              |
    | GetLayout           | ()                    | LayoutReply          | "openwoot/layout"                       |
    | SetLayout           | Layout                | Done                 | "openwoot/layout/set"                   |
    | GetKeyGeometry      | ()                    | KeyGeometryReply     | "openwoot/geometry"                     |
    | GetDefaultKeymap    | KeymapLayer           | KeymapReply          | "openwoot/default-keymap"               |
    | GetGlobalSettings   | ()                    | GlobalSettingsReply  | "openwoot/global"                       |
    | SetGlobalSettings   | GlobalSettings        | Done                 | "openwoot/global/set"                   |
    | GetProfiles         | ()                    | ProfilesReply        | "openwoot/profiles"                     |
    | CreateProfile       | ProfileNamespace      | CreatedProfileReply  | "openwoot/profiles/create"              |
    | DeleteProfile       | Slot                  | Done                 | "openwoot/profiles/delete"              |
    | ActivateProfile     | Slot                  | Done                 | "openwoot/profiles/activate"            |
    | SwapProfiles        | ProfileSwap           | Done                 | "openwoot/profiles/swap"                |
    | MoveProfile         | ProfileMove           | Done                 | "openwoot/profiles/move"                |
    | GetProfileState     | ()                    | ProfileStateReply    | "openwoot/profiles/state"               |
    | SelectLinkedProfile | ProfileSelection      | Done                 | "openwoot/profiles/select-linked"       |
    | GetAppLinkCount     | Slot                  | AppLinkCountReply    | "openwoot/profile/app-links"            |
    | GetAppLink          | AppLinkRef            | AppLinkReply         | "openwoot/profile/app-link"             |
    | SetAppLink          | AppLinkChange         | Done                 | "openwoot/profile/app-link/set"         |
    | DeleteAppLink       | AppLinkRef            | Done                 | "openwoot/profile/app-link/delete"      |
    | GetProfileInfo      | Slot                  | ProfileInfoReply     | "openwoot/profile/info"                 |
    | SetProfileInfo      | ProfileInfoChange     | Done                 | "openwoot/profile/info/set"             |
    | GetProfileRevision  | Slot                  | ProfileRevisionReply | "openwoot/profile/revision"             |
    | GetKeySettings      | Slot                  | KeySettingsReply     | "openwoot/profile/keys"                 |
    | SetKeySettings      | KeySettingsChange     | Done                 | "openwoot/profile/keys/set"             |
    | GetKeymap           | LayerRef              | KeymapReply          | "openwoot/profile/keymap"               |
    | SetKeymap           | KeymapChange          | Done                 | "openwoot/profile/keymap/set"           |
    | GetKeymapLink       | Slot                  | KeymapLinkReply      | "openwoot/profile/keymap/link"          |
    | SetKeymapLink       | KeymapLinkChange      | Done                 | "openwoot/profile/keymap/link/set"      |
    | GetKeyOverrides     | Slot                  | KeyOverridesReply    | "openwoot/profile/overrides"            |
    | SetKeyOverrides     | KeyOverridesChange    | Done                 | "openwoot/profile/overrides/set"        |
    | GetAdvancedKeys     | Slot                  | AdvancedKeysReply    | "openwoot/profile/advanced"             |
    | SetAdvancedKeys     | AdvancedKeysChange    | Done                 | "openwoot/profile/advanced/set"         |
    | GetGamepadMapping   | Slot                  | GamepadMappingReply  | "openwoot/profile/gamepad/mapping"      |
    | SetGamepadMapping   | GamepadMappingChange  | Done                 | "openwoot/profile/gamepad/mapping/set"  |
    | GetGamepadSettings  | Slot                  | GamepadSettingsReply | "openwoot/profile/gamepad"              |
    | SetGamepadSettings  | GamepadSettingsChange | Done                 | "openwoot/profile/gamepad/set"          |
    | GetLighting         | Slot                  | LightingReply        | "openwoot/profile/lighting"             |
    | SetLighting         | LightingChange        | Done                 | "openwoot/profile/lighting/set"         |
    | GetKeyColors        | LayerRef              | KeyColorsReply       | "openwoot/profile/colors"               |
    | SetKeyColors        | KeyColorsChange       | Done                 | "openwoot/profile/colors/set"           |
    | GetLightbar         | Slot                  | LightbarReply        | "openwoot/profile/lightbar"             |
    | SetLightbar         | LightbarChange        | Done                 | "openwoot/profile/lightbar/set"         |
    | GetGlobalLightbar   | ()                    | GlobalLightbarReply  | "openwoot/lightbar"                     |
    | SetGlobalLightbar   | GlobalLightbar        | Done                 | "openwoot/lightbar/set"                 |
    | GetLightbarColors   | ()                    | LightbarColorsReply  | "openwoot/lightbar/colors"              |
    | GetHostSignals      | ()                    | HostSignalsReply     | "openwoot/signals"                      |
    | SetHostSignals      | HostSignals           | Done                 | "openwoot/signals/set"                  |
    | GetSwitches         | ()                    | SwitchesReply        | "openwoot/switches"                     |
    | SetSwitches         | Switches              | Done                 | "openwoot/switches/set"                 |
    | GetSwitchModels     | ()                    | SwitchModelsReply    | "openwoot/switches/models"              |
    | SetKeyReports       | bool                  | Done                 | "openwoot/key-reports/set"              |
    | GetGamepadState     | ()                    | GamepadStateReply    | "openwoot/gamepad"                      |
    | SetGamepadDetection | bool                  | Done                 | "openwoot/gamepad-detection/set"        |
    | GetSensorValues     | SensorReading         | SensorValuesReply    | "openwoot/sensors"                      |
    | SetRawScanning      | bool                  | Done                 | "openwoot/sensors/raw-scanning/set"     |
    | SetHostKeyColor     | HostKeyColor          | Done                 | "openwoot/host-lighting/key/set"        |
    | SetHostColors       | HostColors            | Done                 | "openwoot/host-lighting/set"            |
    | ResetHostKeyColor   | KeyPosition           | Done                 | "openwoot/host-lighting/key/reset"      |
    | ResetHostColors     | ()                    | Done                 | "openwoot/host-lighting/reset"          |
    | SetHostLightbar     | HostLightbar          | Done                 | "openwoot/host-lighting/lightbar/set"   |
    | ResetHostLightbar   | ()                    | Done                 | "openwoot/host-lighting/lightbar/reset" |
    | LeaveHostControl    | ()                    | Done                 | "openwoot/host-lighting/leave"          |
    | GetLedFrame         | ()                    | LedFrameReply        | "openwoot/leds"                         |
    | GetMacros           | ()                    | MacrosReply          | "openwoot/macros"                       |
    | GetMacro            | u8                    | MacroReply           | "openwoot/macro"                        |
    | SetMacro            | MacroChange           | Done                 | "openwoot/macro/set"                    |
    | ClearMacro          | u8                    | Done                 | "openwoot/macro/clear"                  |
    | Restart             | ()                    | ()                   | "openwoot/restart"                      |
    | Save                | ()                    | Done                 | "openwoot/save"                         |
    | Revert              | ()                    | Done                 | "openwoot/revert"                       |
    | FactoryReset        | ()                    | Done                 | "openwoot/factory-reset"                |
}
