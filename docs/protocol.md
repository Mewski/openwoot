# USB face and configuration channel

What a host sees when a keyboard runs openwoot: the USB device, its five interfaces and their reports (the "USB face", kept close to the stock firmware so that operating systems, games and the Wooting Analog SDK keep working), and the configuration channel that `openwoot` and other host software use to talk to the firmware.

| Part | Code |
| --- | --- |
| device and interfaces | `crates/openwoot/src/usb/` (`usb::build`, `usb::Face`, `usb::HostState`, the HID class request handler) |
| report descriptors and typed reports | `crates/openwoot/src/hid/` (the crate-private descriptor builder in `hid/descriptor.rs`, one module per report, re-exported by `hid`) |
| configuration channel, firmware side | `crates/openwoot/src/protocol/` (`protocol::Channel`, `protocol::Handler`, `protocol::UpdateMode` and `protocol::Restart`, the stock identification responder with `protocol::StockIdentity`); the stock feature-report queue `usb::FeatureRequests`; the hand-offs to the other tasks `lighting::HostControl` (host LED control) and `scan::SensorReadout` (sensor readout, raw scanning) |
| messages, framing, host side | `crates/openwoot-protocol/` |

Everything above is generic over `embassy_usb_driver::Driver` and runs on the host; the chip HALs provide the drivers (USBHS with an external ULPI PHY on the Wooting 80HE, USBD on the Wooting 60HE+).

## Device

| Field | Value |
| --- | --- |
| `bcdUSB` | `0x0200` |
| class / subclass / protocol | 0 / 0 / 0 (defined per interface, no interface association descriptors) |
| `bMaxPacketSize0` | 64 |
| `idVendor` | the board's vendor id (`0x31e3`) |
| `idProduct` | the board's product id base plus the USB mode: `31e3:1400`, `31e3:1401`, `31e3:1402` on the Wooting 80HE, `31e3:1320`, `31e3:1321`, `31e3:1322` on the Wooting 60HE+ |
| `bcdDevice` | `0x0250` with the N-key rollover keyboard report, `0x0251` with the boot (6-key) report |
| strings | 1 manufacturer, 2 product, 3 serial number, all from the board (`usb::Identity`); language id `0x0409` |
| configuration | one, value 1, `bmAttributes` `0xa0` (bus powered, remote wakeup), `bMaxPower` 200 (400 mA) |

### Settings that select the face

`usb::Face` is read at boot and decides the descriptors; a change takes effect after a reset.

| Setting | Values | Effect |
| --- | --- | --- |
| `mode` (`usb::Mode`) | `Xinput`, `Gamepad`, `NoGamepad` (default) | what interface 0 is; added to the product id (0, 1, 2) |
| `keyboard` (`usb::KeyboardFormat`) | `NKeyRollover` (default), `Boot` | report descriptor and endpoint of interface 1, `bcdDevice` |
| `speed` (`usb::Speed`) | `High` (default), `Full` | the link speed the descriptors are built for: the Wooting 80HE runs at high speed unless full speed is forced, the Wooting 60HE+ is always full speed |
| `gamepad_polling` (`usb::PollingRate`) | 4 ms, 1 ms (default), 125 µs | `bInterval` of the interface-0 endpoints in the XInput and gamepad modes |

`bInterval` counts 1 ms frames at full speed and 2^(`bInterval` - 1) microframes of 125 µs at high speed:

| Polling rate | High speed | Full speed |
| --- | --- | --- |
| 4 ms | 6 (4 ms) | 4 (4 ms) |
| 1 ms | 4 (1 ms) | 1 (1 ms) |
| 125 µs | 1 (125 µs) | 1 (1 ms) |

## Interfaces

| Interface | Purpose | Class / subclass / protocol | Endpoints (address, size in bytes, `bInterval`) | Report descriptor |
| --- | --- | --- | --- | --- |
| 0, `Xinput` | Xbox 360 wired controller | `0xff` / `0x5d` / `0x01` with a 17-byte class descriptor (type `0x21`) | `0x81` IN 32 and `0x01` OUT 32, polling-rate interval | none |
| 0, `Gamepad` | HID gamepad | 3 / 0 / 0 | `0x81` IN 32, polling-rate interval | `hid::GAMEPAD_DESCRIPTOR` |
| 0, `NoGamepad` | placeholder, never sends | 3 / 0 / 0 | `0x81` IN 8, interval 10 | `hid::PLACEHOLDER_DESCRIPTOR` |
| 1 | keyboard | 3 / 1 / 1 (boot keyboard) | `0x82` IN 28, interval 1 (N-key rollover); `0x82` IN 8, interval 10 (boot) | `hid::NKRO_DESCRIPTOR` or `hid::BOOT_KEYBOARD_DESCRIPTOR` |
| 2 | configuration channel | 3 / 0 / 0 | `0x03` OUT 64 then `0x83` IN 64, interval 1 | `hid::CHANNEL_DESCRIPTOR` |
| 3 | system control, consumer control, mouse | 3 / 0 / 0 | `0x84` IN 8, interval 4 | `hid::CONTROLS_DESCRIPTOR` |
| 4 | key states for the Wooting Analog SDK | 3 / 0 / 0 | `0x85` IN 64, interval 1 | `hid::KEY_STATE_DESCRIPTOR` |

Every HID interface has a HID descriptor with `bcdHID` 1.11, country 0 and one report descriptor. Resulting polling periods: keyboard 125 µs at high speed and 1 ms at full speed (N-key rollover), 64 ms and 10 ms (boot report, as stock); configuration channel and key states 125 µs and 1 ms; interface 3 1 ms and 4 ms.

### Control requests

| Request | Answer |
| --- | --- |
| `GET_DESCRIPTOR` HID (`0x21`) or report (`0x22`) on a HID interface | the HID descriptor or the report descriptor of that interface |
| `SET_IDLE`, `GET_IDLE` | stored per interface, reset by a bus reset |
| `SET_PROTOCOL`, `GET_PROTOCOL` | stored per interface; on interface 1 published as `HostState::keyboard_protocol` (boot or report) |
| `SET_REPORT` output on interface 1 | the lock lights, published as `HostState::keyboard_leds` (bit 0 Num Lock, 1 Caps Lock, 2 Scroll Lock, 3 Compose, 4 Kana); cleared by a bus reset and when VBUS is removed |
| `SET_REPORT` feature on interface 2 | a stock feature command, queued in `HostState::feature_requests` for the [stock identification responder](#stock-identification-responder); stalled when the report id is not 1, when the data is shorter than the 7-byte command (with or without the leading id byte), or when the queue is full; longer data uses its first 7 command bytes, as stock |
| `SET_REPORT` output or input on interface 2 | accepted and ignored (stock output-report commands; none is supported) |
| `GET_REPORT` on interface 2 | the report id byte only, as stock; report id 0 and a `wLength` below 2 stall |
| string `0xee` in the XInput mode | Microsoft OS 1.0 string `MSFT100` with request code `0xb0` |
| vendor request `0xb0`, `wIndex` 4, in the XInput mode | 40-byte extended compat ID that gives interface 0 the compatible id `XUSB10`, so Windows binds its XInput driver without an INF file |

## Reports

Report descriptors are built at compile time by the descriptor builder in `hid/descriptor.rs` (a `const fn` item builder that encodes each item in its smallest size and fails compilation on unbalanced collections or overflow). Each report is a typed struct implementing `hid::InputReport` (`ID`, `LEN`, `to_bytes`), and `usb::write_report` sends it. The host tests parse every descriptor and check that each report's size equals its struct.

| Report | Bytes (little endian) |
| --- | --- |
| `hid::BootReport` (interface 1, boot) | modifiers (bit n = usage `0xe0` + n), 0, six key usages |
| `hid::NkroReport` (interface 1) | modifiers, then a 27-byte bitmap: usages `0x04`..`0x31`, `0x33`..`0x9b`, `0x9d`..`0xa4`, `0xb0`..`0xdd`, each group padded to a whole byte; one table (`NKRO_KEY_GROUPS` in `hid/keyboard.rs`) builds both the descriptor and the bit positions |
| keyboard output (interface 1) | one byte, five LED bits and three padding bits |
| `hid::SystemReport` (id 1) | id, control: 0 none, 1 power down, 2 sleep, 3 wake up (usages `0x81`..`0x83`) |
| `hid::ConsumerReport` (id 2) | id, three 16-bit consumer usages (1..`0x02a2`, 0 none) |
| `hid::MouseReport` (id 3) | id, five button bits and three padding bits, relative X and Y (-127..127) |
| `hid::GamepadReport` (interface 0, `Gamepad`) | 12 button bits and 4 padding bits, X, Y, Z, Rz, Rx, Ry (0..`0x7ffe`, centre `0x3fff`), throttle, elevator, accelerator, rudder, trigger (0..`0x7fff`), hat (1..8 clockwise from up, 0 centred) |
| `xinput::Report` (interface 0, `Xinput`) | `0x00`, `0x14`, buttons (16 bits), left and right trigger (8 bits each), left X, left Y, right X, right Y (signed 16 bits), six zero bytes |
| XInput output (`xinput::LedPattern`) | `0x01 0x03 <pattern>`: patterns 2..5 and 6..9 select player 1..4 (`usb::HostState::xinput_player`, forgotten when the host configures or deconfigures the device); the player picks the corner of the gamepad detection pattern |
| `hid::KeyStateReport` (interface 4) | 16 entries of 4 bytes: key position, usage code, then a 16-bit word: bit 0 pressed, bits 2..5 kind (0 keyboard, 3 consumer, 4 function, 5 gamepad, 6 advanced), bits 6..15 travel (0..1023); unused entries are 0 |

## Configuration channel

The channel uses interface 2 (usage page `0xff55`, usage 1) with the stock report ids and sizes: ids 1 to 6 carry 32, 62, 254, 510, 1022 and 2046 bytes (without the report id byte) as input and output reports; id 1 also declares the 7-byte stock feature report, which carries the stock commands of the [identification responder](#stock-identification-responder). The host writes output reports, which travel over the interrupt OUT endpoint `0x03` (hidapi `write`); the firmware answers with input reports on `0x83` (hidapi `read`). Every report ends with a short packet, so no zero-length packets are needed.

### Report envelope

```
report id | 0x4f 0x57 ("OW") | frame length (u16, little endian) | frame | zero padding
```

A frame goes into the smallest report that holds it (up to 2042 bytes in report 6). Stock traffic starts with `d1 da` or `ee` and never with `OW`, so the two cannot be confused (`openwoot_protocol::report::parse` returns `report::Contents::Foreign` for it). A report that starts with `OW` but cuts off the frame length, carries a report id other than 1 to 6, or names a frame longer than its report or its data is malformed (`Error::MalformedReport`) and gets no answer. A body must be exactly one value: `decode_response` refuses a reply with bytes behind its response (`Error::Decode`) and takes only replies with an 8-byte key and a 2-byte sequence number, the sizes the host sends.

### Frame

Each frame is one message in the format of `postcard-rpc` 0.12 (only its core: keys, header and standard errors; its transports are not used):

```
header byte | key (1, 2, 4 or 8 bytes) | sequence number (1, 2 or 4 bytes, little endian) | postcard body
```

The header byte holds the key size in bits 7..6 (`00` 1, `01` 2, `10` 4, `11` 8 bytes), the sequence number size in bits 5..4 (`00` 1, `01` 2, `10` 4 bytes) and the header version 0 in bits 3..0. A key is the hash of an endpoint's path and the schema of its request (for requests) or response (for responses), so a request whose type changed reaches no handler. The body is the request or response encoded with `postcard` 1.2.

The host sends requests with 8-byte keys and 2-byte sequence numbers (`openwoot_protocol::encode_request`). The firmware answers each request with one response that repeats the sequence number and uses a key of the request's key size: the endpoint's response key, or the error key (path `error`, type `postcard_rpc::standard_icd::WireError`, for example `UnknownKey` for a request it does not know). `openwoot_protocol::decode_response` returns the response, the device's error, or nothing for reports that answer something else. The firmware serves one request at a time; the host waits for the answer before it sends the next request. A report ends at the short packet or at the length its report id declares, whichever comes first. A transfer whose next packet does not arrive within 100 ms is dropped. `GetSensorValues` and the host-lighting endpoints answer `WireError::FailedToSpawn` when the key task or the lighting task does not take the request within 500 ms.

### Endpoints

Every endpoint is declared once in `openwoot_protocol::endpoints` (one table, collected in `endpoints::LIST`). `endpoints::LIST` also holds the two standard endpoints that `postcard-rpc` adds to every table, `postcard-rpc/ping` and `postcard-rpc/schemas/get`; the firmware serves neither and answers them with `UnknownKey`. `Done` stands for `Result<(), Rejection>`; every other configuration response is `Result<T, Rejection>` with the type shown.

| Endpoint | Path | Request | Response |
| --- | --- | --- | --- |
| `GetVersion` | `openwoot/version` | `()` | `Version { protocol: u16, firmware: FirmwareVersion { major: u16, minor: u16, patch: u16 } }` |
| `EnterUpdateMode` | `openwoot/update-mode` | `()` | `()`; once the reply has been sent and 50 ms have passed, so that the host has collected it, the firmware hands over to the factory bootloader (`UpdateMode::enter_update_mode` with option byte 0), which stays in update mode for one start; if the reply cannot be sent (the host reset the bus), nothing is handed over |
| `Restart` | `openwoot/restart` | `()` | `()`; once the reply has been sent and 50 ms have passed, the firmware restarts (`Restart::restart` with option byte 0, so the bootloader starts the firmware again); if the reply cannot be sent, nothing restarts |
| `GetDiagnostics` | `openwoot/diagnostics` | `()` | `diagnostics::Report`: scan cycles, recoveries and converter overruns (`None` where the chip has no overrun flag), key passes and published key reports, the applied profile with profile requests and switches, LED driver and lighting state with frames and page writes, completed transfers and errors by kind with the last error of the LED bus (I2C) and the flash bus (SPI), settings loads, stores and failures, USB configured, suspended, negotiated speed and bus resets, the stack size and its deepest use since reset (`Stack { size, peak }` in bytes, from the stack painted at reset); all counters wrap at `u32::MAX`; firmware built without a readout answers `UnknownKey` |
| `GetCapabilities` | `openwoot/capabilities` | `()` | `configuration::Capabilities` |
| `GetDeviceSettings`, `SetDeviceSettings` | `openwoot/device`, `…/set` | `()`, `device::Settings` | `device::Settings`, `Done` |
| `GetLayout`, `SetLayout` | `openwoot/layout`, `…/set` | `()`, `device::Layout` | `device::Layout`, `Done` |
| `GetKeyGeometry` | `openwoot/geometry` | `()` | `geometry::Keys`: the keycaps of the layout in effect (the one `GetLayout` reports), see the geometry part of the [configuration model](#configuration-model); a keyboard whose board hands over no geometry answers `Unsupported` |
| `GetGlobalSettings`, `SetGlobalSettings` | `openwoot/global`, `…/set` | `()`, `global::Settings` | `global::Settings`, `Done` |
| `GetProfiles` | `openwoot/profiles` | `()` | `profile::Summary { count, active }` |
| `CreateProfile` | `openwoot/profiles/create` | `()` | `profile::Slot` of the new profile |
| `DeleteProfile`, `ActivateProfile` | `openwoot/profiles/delete`, `…/activate` | `profile::Slot` | `Done` |
| `GetProfileInfo`, `SetProfileInfo` | `openwoot/profile/info`, `…/set` | `Slot`, `Change<profile::Info>` | `profile::Info`, `Done` |
| `GetProfileRevision` | `openwoot/profile/revision` | `Slot` | `u32` |
| `GetKeySettings`, `SetKeySettings` | `openwoot/profile/keys`, `…/set` | `Slot`, `Change<keys::Settings>` | `keys::Settings`, `Done` |
| `GetKeymap`, `SetKeymap` | `openwoot/profile/keymap`, `…/set` | `LayerRef`, `LayerChange<keys::Keymap>` | `keys::Keymap`, `Done` |
| `GetKeyOverrides`, `SetKeyOverrides` | `openwoot/profile/overrides`, `…/set` | `Slot`, `Change<keys::Overrides>` | `keys::Overrides`, `Done` |
| `GetAdvancedKeys`, `SetAdvancedKeys` | `openwoot/profile/advanced`, `…/set` | `Slot`, `Change<advanced::Keys>` | `advanced::Keys`, `Done` |
| `GetGamepadMapping`, `SetGamepadMapping` | `openwoot/profile/gamepad/mapping`, `…/set` | `Slot`, `Change<gamepad::Mapping>` | `gamepad::Mapping`, `Done` |
| `GetGamepadSettings`, `SetGamepadSettings` | `openwoot/profile/gamepad`, `…/set` | `Slot`, `Change<gamepad::Settings>` | `gamepad::Settings`, `Done` |
| `GetLighting`, `SetLighting` | `openwoot/profile/lighting`, `…/set` | `Slot`, `Change<lighting::Settings>` | `lighting::Settings`, `Done` |
| `GetKeyColors`, `SetKeyColors` | `openwoot/profile/colors`, `…/set` | `LayerRef`, `LayerChange<Option<lighting::KeyColors>>` | `Option<lighting::KeyColors>`, `Done` |
| `GetLightbar`, `SetLightbar` | `openwoot/profile/lightbar`, `…/set` | `Slot`, `Change<lightbar::Settings>` | `lightbar::Settings`, `Done` |
| `GetGlobalLightbar`, `SetGlobalLightbar` | `openwoot/lightbar`, `…/set` | `()`, `lightbar::Entries` | `lightbar::Entries`, `Done` |
| `GetHostSignals`, `SetHostSignals` | `openwoot/signals`, `…/set` | `()`, `lightbar::HostSignals` | `lightbar::HostSignalIds`, `Done` |
| `GetSwitches`, `SetSwitches` | `openwoot/switches`, `…/set` | `()`, `switches::Settings` | `switches::Settings`, `Done` |
| `GetSwitchModels` | `openwoot/switches/models` | `()` | `switches::Models` |
| `SetKeyReports` | `openwoot/key-reports/set` | `bool` (enabled) | `Done`; turns key reports off and on: while off, keys send no keyboard, controls or key-state reports, Fn actions do not run and advanced keys rest (a latched toggle is let go when key reports go off); the gamepad report keeps following the keys; runtime state (`usb::HostState`), not stored, back on when the host configures the USB device |
| `SetGamepadDetection` | `openwoot/gamepad-detection/set` | `bool` (enabled) | `Done`; starts and stops gamepad detection: the gamepad interface sends the fixed detection pattern (XInput: the left stick in the corner of the player) instead of the key state; stopped when the host configures the USB device; runtime state (`usb::HostState`), not stored |
| `GetSensorValues` | `openwoot/sensors` | `sensors::Reading` (`Travel`, `Value`, `RestLevel`, `BottomLevel`) | `sensors::Values`: one `Option<u16>` per key in key order (row by row over the key matrix), `None` for keys without a sensor; `Travel` is the key travel 0..4095, `Value` the sensor value (the raw converter value while raw scanning is on), `RestLevel` and `BottomLevel` the calibrated levels of the current scan mode; the key task fills it after its next scan cycle |
| `SetRawScanning` | `openwoot/sensors/raw-scanning/set` | `bool` (enabled) | `Done`; while on, the scan keeps the raw converter value of every sensor and evaluates no key; ended when the host resets or reconfigures the USB device; runtime state (`scan::SensorReadout`), not stored |
| `SetHostKeyColor`, `ResetHostKeyColor` | `openwoot/host-lighting/key/set`, `…/key/reset` | `host_lighting::KeyColor { key: keys::Position, color }`, `keys::Position` | `Done`; setting a colour hands the key LEDs to the host (effects, Fn-layer colours and lock indicators stop) and paints one key; resetting gives one key its profile colour back without leaving host control, and does nothing outside host control |
| `SetHostColors`, `ResetHostColors` | `openwoot/host-lighting/set`, `…/reset` | `lighting::KeyColors` (one colour per key, key order), `()` | `Done`; paints every key (and takes the LEDs), or gives every key its profile colour back while the host keeps the LEDs |
| `LeaveHostControl` | `openwoot/host-lighting/leave` | `()` | `Done`; gives the LEDs back to the profile, ending RGB SDK control; also happens when the host resets or reconfigures the USB device; runtime state (`lighting::HostControl`), not stored |
| `Save`, `Revert`, `FactoryReset` | `openwoot/save`, `openwoot/revert`, `openwoot/factory-reset` | `()` | `Done` |

`PROTOCOL_VERSION` is 1. A key is a hash of the path and the schema of the type (its shape with field and variant names; type names are not hashed), so a host and a firmware built with different types for a path do not reach each other's handlers; a test checks that no two endpoints share a request or response key. Capacities are not part of the schema (a `heapless::Vec<T, N>` hashes as a sequence of `T`): a changed `MAX_*` keeps every key, and a value longer than the receiver's capacity fails to decode (`DeserFailed` on the firmware, `Error::Decode` on the host).

### Configuration model

The configuration is a set of typed parts, each a `no_std` serde type in `openwoot_protocol`, sized by `heapless` capacities so that the largest value of every part fits one frame (tested). Storage keeps its own records of the same types.

| Part | Type | Holds |
| --- | --- | --- |
| capabilities | `configuration::Capabilities` | key matrix rows and columns, number of profile slots, layouts the board can have, light-bar LEDs (0 = none), host signals, number of switch models, edition |
| device | `device::Settings` | USB mode (`Xinput`, `Gamepad`, `NoGamepad`), keyboard report (`NKeyRollover`, `Boot`), speed (`High`, `Full`), gamepad polling (4 ms, 1 ms, 125 µs); converted to `usb::Face` by `From`; takes effect at the next start |
| layout | `device::Layout` | `Ansi`, `Iso`, `Jis`; must be one of `Capabilities::layouts`; `GetLayout` reports the layout in effect, `SetLayout` chooses the layout of the next start |
| geometry | `geometry::Keys` | read only, the board's own data (`GEOMETRY` in `boards/<board>/src/keys.rs`, one table per layout variant, ported from the stock Wootility's layout tables), so that a host draws the keyboard without knowing the board: one `Option<geometry::Key>` per key in key order, `None` where the layout in effect has no key; a `geometry::Key { x, y, width, height, shape }` is a keycap in hundredths of a key unit (`geometry::UNIT` = 100, so a 1.25u key is 125 wide), `x` and `y` its top-left corner measured from the top-left corner of the key area (no case border); `shape` is `Rectangle` or `IsoEnter`, the stepped ISO enter, whose box is its bounding box (150 × 200): the upper row (one unit high) spans the full width, below it the key starts `geometry::ISO_ENTER_STEP` (25) right of `x`; `Key::contains` tests a point against that outline. Keys are the keycaps with a sensor: the space bar is one key even where further LEDs sit under it (`5,4`, `5,5`, `5,7` and `5,8`, on the JIS Wooting 80HE `5,5` and `5,7`), and those LEDs have no entry |
| global | `global::Settings` | idle dimming (`lighting::Idle`: dim after, off after, off enabled; `None` = never; a time of 0 minutes counts as 1 minute), Num Lock indicator inverted, release-rebound filter |
| profile info | `profile::Info` | name (up to 32 bytes of UTF-8), colour |
| key settings | `keys::Settings` | profile-wide actuation point, rapid trigger (enabled, press and release sensitivity, continuous), tap duration (1, 20, 40 ms), scan mode (single or parallel), keyboard keys in the gamepad modes |
| keymap | `keys::Keymap` | one `keys::Action` per key of one layer (`Base`, `One`, `Two`, `Three`) |
| overrides | `keys::Overrides` | per key: actuation point or the profile's, rapid trigger `Profile`, `Off`, `On` (profile parameters) or `With(RapidTrigger)` |
| advanced keys | `advanced::Keys` | up to 40 entries `{ key, layer, behaviour }`: dynamic keystroke (four strokes, each an action with press / bottom-out / lift / release events `Nothing`, `Press`, `Release`, `Tap`, and the bottom-out point), mod-tap (tap and hold action, hold time in ms, interrupt `Auto`, `Ignore`, `Hold`, `Tap`), toggle (action), depth priority and SOCD with the other key and the resolution `Both`, `Neither`, `First`, `Second`, `LastPressed` |
| gamepad mapping | `gamepad::Mapping` | per key an optional `gamepad::Control` (buttons, d-pad, sticks by direction, triggers, the further HID axes, hat), keys with a control exclusive to the gamepad |
| gamepad settings | `gamepad::Settings` | four-point response curve (first point: dead zone, last: saturation), circular or square normalisation, single-direction axes, left-stick up bias in percent |
| lighting | `lighting::Settings` | brightness, lock-indicator colours (Caps, Scroll, Num, layer lock), up to 6 effect layers (`lighting::Effect`: rainbow wave, scanner, breathing, hue cycle, key ripple, reactive fade, level meter, analog spotlight, analog glow, analog RGB fill, key test, simplex noise, polar gradient) |
| key colours | `Option<lighting::KeyColors>` | per layer one `Rgb` per key; `None` = no own map on that layer (on the base layer: unlit) |
| light bar | `lightbar::Settings` | enabled, up to 4 entries (optional condition of up to two groups of two terms: trigger on a signal change, or comparison of a signal with a signal or constant, optional hold time; effect meter, profile indicator, mirror, blink, solid, custom), use the global entries, brightness |
| global light bar | `lightbar::Entries` | up to 4 entries drawn before every profile's entries |
| host signals | `lightbar::HostSignals` | values `{ id, value }` of host-defined signals (up to 20); `GetHostSignals` lists the ids the light bar refers to |
| switches | `switches::Settings` | keyboard-wide, not per profile: per key an optional switch (`switches::Switch`: model, travel offset in 0.05 mm, 0 = none) and an optional default; a key without its own switch uses the default, a key without either the switch the unit was built with (model 1 on the TenZ edition, 0 otherwise); `GetSwitchModels` lists the travel (0.05 mm) and group of the first `Capabilities::switch_models` models |

Keys are addressed by `keys::Index`, `row * key_columns + column` of the board's key matrix; per-key lists hold exactly `key_rows * key_columns` entries. Distances (`keys::Distance`) are fractions of the full travel of the key's switch in 1/65535. Colours with transparency (`lighting::Color`) carry an alpha of 0 to 255 (255 opaque).

Actions (`keys::Action`): `None`, `Keyboard(usage)` (the usages the N-key rollover report carries, `Action::KEYBOARD_USAGES`: `0x04`..`0x31`, `0x33`..`0x9b`, `0x9d`..`0xa4`, `0xb0`..`0xdd`, and the modifiers `0xe0`..`0xe7`; `0x32`, `0x9c` and `0xa5`..`0xaf` are refused because that report has no bit for them), `Consumer(usage)` (`0x0001`..`0x02a2`), `System` (power down, sleep, wake up), `Mouse` (left, right, middle, back, forward), `Gamepad(control)`, `Layer(layer)` (momentary, not the base layer), `Function` (brightness up and down, profile, next profile, toggle mode profile, toggle key disable, layer lock).

Semantics:

- A `Set*` request applies at once: to the running keyboard when it addresses the active profile, otherwise to that profile. Changes to the active profile and to the global, device, global light-bar, switch and layout settings are stored by `Save`, which writes every such change since the last save, and `Revert` drops them. A `Set*` that addresses an inactive profile is stored at once and counts in that profile's revision, so `Save` and `Revert` do not concern it. `FactoryReset` returns every setting and profile to the board's factory defaults (stored at once).
- The firmware validates every request that carries a value against `Capabilities` before it reaches the keyboard (`configuration::Validate`, implemented by every part of the model down to single keys, actions and distances) and answers `Err(Rejection::Invalid(..))` without changing anything. Because validation comes first, setting a part the keyboard has no capability for is refused as invalid (`SetLightbar`, `SetGlobalLightbar` with `Invalid(NoLightbar)`, `SetHostSignals` with `Invalid(NoHostSignals)`), while reading it answers `Unsupported`. `Rejection` also reports a missing profile, a profile that cannot be deleted, the profile limit, a storage error, and `Unsupported` for parts a configuration does not implement; `profile::Configurator` implements every part, the geometry once the board hands its tables to `Configurator::with_geometry`. `Rejection::Locked` is reserved; no request produces it yet.
- Firmware side: `protocol::Handler` decodes, validates and hands each request to `protocol::Configuration`, a trait with one async method per part; every method answers `Unsupported` unless implemented, and `()` implements none (`Handler::new`). `profile::Configurator` implements it over `profile::Model` (`Handler::with_configuration`) without changes to the handler.
- Profiles: a fresh keyboard has the board's factory profiles (`boards/<board>/src/profiles.rs`, `profile::Factory`: two on a regular unit, Typing and Rapid, with their effect layers, colour maps, key settings, names and colours); every part a profile has not stored is its factory part of that slot, so nothing is written at the first start. `GetProfiles` counts the existing profiles (up to `Capabilities::profiles`); `CreateProfile` adds the next slot (`ProfileLimitReached` when full), `DeleteProfile` removes only the last one and never profile 0 (`ProfileProtected`), and a deleted active profile hands over to the new last one. `ActivateProfile`, the Mode key (`ToggleModeProfile`: from profile 0 to the last other active profile, profile 1 at first; from any other back to 0) and the profile keys switch through one activation that saves the unsaved changes, applies every part (keys, lighting, colour maps) and stores the active slot; the switch applies even when the store fails, and the changes it could not store are dropped (the request answers `StorageInaccessible`). `GetProfileRevision` counts the stores of a profile's parts.
- Global and device settings: `SetGlobalSettings` applies idle dimming, the Num Lock polarity and the release-rebound filter at once; `SetDeviceSettings` takes effect at the next start, where the stored settings choose the USB face (a stored `High` speed keeps the board's speed, `Full` forces full speed). Both are stored by `Save`. `SetSwitches` applies at once: the next key pass fits every key's sensor with its switch (`analog::Switch`: travel curve, bottom level, learn strength; a fitted sensor forgets its learned bottom-out), copies the switch's minimum distance and travel limit into the key engine and recomputes the profile's key settings, overrides and advanced keys from them (advanced keys restart, so a latched toggle is let go); setting the switches every key already has changes nothing. `SetLayout` stores nothing until `Save`; the next start reads the stored layout before it chooses the sensor map, keymap and LED map (`storage::LayoutOverride`), and `GetLayout` keeps reporting the layout in effect until then. A brightness key stores the new level into the profile's stored lighting settings two seconds after the last step; other lighting changes that are not saved stay unsaved, and a level stepped less than two seconds before a profile switch is not kept for the profile that was left.
- Light bar and host signals: `SetLightbar` on the active profile and `SetGlobalLightbar` redraw the light bar at once and are stored by `Save`. A profile without stored light-bar settings shows the global entries at the key brightness, or at the brightness its factory preset gives (the TenZ edition's profiles). `GetHostSignals` lists the ids of the host signals the bound entries refer to, in the order they were bound; `SetHostSignals` sets values by id. The keyboard keeps the newest 20 values of ids that no bound entry refers to, and an entry that binds such an id later starts from its value. Values also survive a re-bind (profile switch, light-bar change, scan mode change). The global and profile entries together bind at most 20 distinct host ids, in bind order (global entries first); later ids are not bound and not listed, and entries that use them do not change. Either request lets meters show host signals from then on, until the host's session ends: a bus reset or a new configuration (the PC restarts, or the keyboard is plugged in again) sets every host signal to 0, the values kept for unbound ids too, so conditions on host signals see 0, and turns host meters off until the host lists or sets the signals again. An app that quits does not reset the bus; it sets its signals to 0 itself if they should not stay. The list changes when the entries are bound again (profile switch, light-bar change, scan mode change); the host reads it again then, since there is no "light bar changed" event.

The configuration has one profile namespace of `Capabilities::profiles` slots. Key colours are 8-bit RGB per key, distances use one 16-bit encoding and the mod-tap hold time is in ms. Light-bar meters and profile indicators draw on light-bar ranges only; there are no effect kinds 12 and 13 and no single-effect form. Rapid-trigger settings are one per-key override list, and the switch profile is set per key. Validation refuses out-of-range values and zero divisors instead of clamping them, and also refuses what the firmware could store but never show: an advanced key bound twice on one layer (a depth-priority or SOCD entry binds its other key too), a light-bar condition that can never hold (`Single` without exactly one group or term, `Any` without a group or term), a trigger duration or hold time of 0 or above 60000 ms (`lightbar::MAX_HOLD_MS`), a key rectangle whose first key lies behind its last, a switch whose travel offset reaches its model's travel (`Invalid::TravelOffsetReachesTravel`; the firmware checks it against its model table, since `Capabilities` carries only the number of models, with `switches::Settings::check_models`, which a host can run on the `GetSwitchModels` list), more than `switches::MAX_TUNED` (16) distinct pairs of model and travel offset among the switches with an offset (`Invalid::TunedSwitchesExceedLimit`, the firmware fits each pair once), and one list of light-bar entries that names more host signals than the keyboard lists (`lightbar::MAX_HOST_SIGNALS`). Two values are still adjusted by the firmware, because they depend on what validation cannot see: an actuation or rapid-trigger distance below the switch's minimum travel is raised to that minimum, and global and profile light-bar entries together that name more than `MAX_HOST_SIGNALS` host signals follow only the first ones. A part a profile never stored is the factory part of its slot, and so is a stored part that fails this validation when it is loaded (keymaps and colour maps layer by layer), for example one stored by a build with other limits.

### Stock identification responder

The official Wootility recognises keyboards by vendor id, product id and usage page `0xff55` and talks to them with stock feature commands. The responder answers the few it needs to recognise the keyboard and to install the stock firmware, so that returning to stock is the normal Wootility update (see [flashing](flashing.md)), and the runtime commands of the Wooting RGB SDK (used by OpenRGB, SignalRGB and similar programs) and of game-controller detection, mapped to the same runtime state as the openwoot endpoints.

A stock feature command is a `SET_REPORT` of feature report 1 on interface 2 (`wValue` `0x0301`): `01 d1 da <command> <a0 a1 a2 a3>` (argument little endian; the command without the report id byte is accepted too, and longer data uses its first 7 command bytes, as stock; another report id or data shorter than 7 command bytes is stalled, as is a command that finds the four-entry queue full). The USB control handler checks and queues it in `HostState::feature_requests` (`usb::FeatureRequests`: four entries, at most three commands, as stock), and `protocol::Channel` answers it on `0x83` as an input report in the smallest report that holds it, zero padded:

```
report id | d1 da | command | status | payload length (u16, little endian) | payload
```

| Command | Argument | Answer (status `0x88` unless stated) |
| --- | --- | --- |
| 0, ping | `a0` | 1 byte: `a0` inverted |
| 1, version | none | `02 01 02`: version 2.1.2, see below |
| 2, reset to update mode | `a0`: option bits for the bootloader (bit 0 high speed, bit 1 ignore the key combination, bit 2 no lighting) | none; the channel calls `UpdateMode::enter_update_mode(a0)` 50 ms after it took the command, so that the `SET_REPORT` status stage completes (`a0` is passed on unchanged); it writes `0x504` and `a0` to the backup registers and resets, as stock |
| 3, serial number, selector 0 | `arg` = 0 | 11 bytes: supplier (u16), year, week, product (u16), revision (u16), product id (u16, at most `0xfffe`), stage; all zero without a serial number |
| 3, selector 1 | `arg` = 1 | 12 bytes: the 96-bit unique id of the microcontroller (`StockIdentity::mcu_id`, read by `hal::flash::unique_id`) |
| 3, selector 2 | `arg` = 2 | the protobuf message `SerialNumber` (fields 1 to 7 always, 9 edition, 10 PCB design, 11 minor revision when present), encoded as the stock firmware encodes it; without a serial number status `0xff` with the 16-bit error -2 (`fe ff`, no device information) |
| 3, selector 3 | `arg` = 3 | 10 bytes: manufacturer and device id of the settings flash (command `0x90`) and its 8-byte unique id (command `0x4b`), `StockIdentity::flash_ids`; zero if the chip does not answer |
| 3, other selectors | | status `0xff`, no payload |
| 19, device settings | none | 7 bytes: byte 0 bit 3 full speed forced (`usb::Speed::Full`), bit 4 boot keyboard report; bytes 1 to 3 zero; byte 4 `0x11` (flash read-out allowed); byte 5 the layout in effect (`StockIdentity::layout`: the layout stored by `SetLayout`, otherwise the Wooting 80HE device-information record or the Wooting 60HE+ bit 0 of the user option byte); byte 6 bits 2..1 USB mode (`usb::Mode`: 0 XInput, 1 gamepad, 2 none) and bits 4..3 gamepad polling (0 4 ms, 1 1 ms, 2 125 µs) |
| 21, keys off; 22, keys on | none | none; key reports off and on, as `SetKeyReports` |
| 30, set one key colour (RGB SDK) | `a3` key (`row << 5 \| column`), `a2` red, `a1` green, `a0` blue | none; takes the key LEDs as `SetHostKeyColor`, also for a key outside the matrix, which then gets status `0xff`; status `0x77` when the lighting task does not take the command within 500 ms |
| 31, reset one key colour (RGB SDK) | `a0` key | none; as `ResetHostKeyColor`; status `0xff` for a key outside the matrix; status `0x77` when the lighting task does not take the command within 500 ms |
| 32, reset all colours (RGB SDK) | none | none; as `LeaveHostControl` (taking the LEDs first, so a shown Fn-layer colour map ends too, as stock); status `0x77` when the lighting task does not take the command within 500 ms |
| 33, take the LEDs (RGB SDK) | none | none; the host owns the key LEDs, all keys still in their current colour; status `0x77` when the lighting task does not take the command within 500 ms |
| 45, start XInput detection; 46, stop | none | none; as `SetGamepadDetection` |
| every other command | | no payload; status `0x66` (unknown) for numbers below 38 and above 88, `0xaa` (unsupported) for 38 to 88, the stock rule for commands without a handler; the same for 21, 22 and 30 to 33 (`0x66`) and 45, 46 (`0xaa`) when the firmware attaches no `usb::HostState` or `lighting::HostControl` to the handler |

A request with another magic gets status `0xff` (under the magic `d1 da`), and a fourth waiting command status `0x77`, both at once as stock does.

The serial number comes from `StockIdentity::serial`, the serial text the board reads from the serial page of the microcontroller (for example `A01B2401W01T00100S01H00300`, a made-up serial: supplier 01, year 24, week 01, product 01, PCB design 00, revision 1, minor revision 00, edition 01, stage H, product id 00300). The official Wootility identifies a keyboard by these fields and finds it again by them after the update, so they must equal what the stock firmware reports.

Version 2.1.2 is a factory version for the official Wootility: it asks for the update to the current stock firmware before anything else, reads no profiles first (openwoot answers none), and resets the stock settings after the update. With the stock version 2.14.1, Wootility would treat the keyboard as up to date and read its profiles.

Stock output-report commands (`d1 da` in an output report on `0x03`) get no answer, as stock. Command 11 of the RGB SDK paints every key: `d1 da 0b` followed by 6 rows of 21 RGB565 colours (little endian, 252 bytes); the columns of the key matrix are used, the others ignored; it takes the key LEDs as `SetHostColors`, and a shorter report is ignored. Other stock output-report commands are ignored, and so are output reports through `SET_REPORT`; `OW` frames on the same reports work as described above.

## Deviations from the stock face

| Topic | Stock firmware | openwoot |
| --- | --- | --- |
| `DEVICE_QUALIFIER` | stalled | answered on the high-speed Wooting 80HE (embassy-usb); stalled on the full-speed-only Wooting 60HE+ by its USB driver, as stock |
| BOS | stalled | answered with an empty BOS (only asked for by hosts when `bcdUSB` is 2.01 or higher) |
| `GET_STATUS` on an interface | stalled | answered with 0 |
| `SET_INTERFACE` with an alternate setting above 0, `SET_CONFIGURATION` above 1, `SET_ADDRESS` 0 | acknowledged | stalled |
| resume | reported at the first start of frame after the wake-up interrupt | reported at the wake-up interrupt |
| configuration channel OUT endpoint | not re-armed until the command is processed | one packet of the next transfer may arrive before the current one is processed (the channel's request/response pairing is the flow control) |
| `SET_REPORT` on interface 2 (feature or output report through the control endpoint) | processed as a stock command, up to 64 (feature) or 2048 (output) bytes | feature reports go to the stock identification responder; output reports are accepted and ignored, so `OW` frames travel only over the interrupt OUT endpoint |
| `GET_REPORT` on interfaces other than 2 | answered with the report id byte only | stalled |
| stock feature commands | 65 commands on the Wooting 80HE, 59 on the Wooting 60HE+ | commands 0, 1, 2, 3 and 19 (version 2.1.2 instead of 2.14.1), 21 and 22, 30 to 33, 45 and 46; the others are answered as unknown or unsupported |
| stock output-report commands | 28 commands on the Wooting 80HE, 22 on the Wooting 60HE+ | command 11 (RGB SDK colour map) over the interrupt OUT endpoint; the others are ignored |
| control transfers longer than 128 bytes | accepted up to 2048 bytes | stalled before the data stage |
| report descriptors | stock byte arrays | same fields; byte encodings differ where items are ordered differently or encoded in their smallest size, for example the boot keyboard's logical maximum 221 is `26 dd 00` where stock writes `25 dd` (which reads as -35) |
| gamepad hat switch | Input flags `0x02` | `0x42` (Null State), so the centred value 0 is declared as "no direction" |
| interface 3 reports | always 7 bytes (id and 6 data bytes) | each report at its own length: 2 bytes (system), 7 (consumer), 4 (mouse) |
| USB mode 3 | interface 0 answers with the keyboard report descriptor | not representable (`usb::Mode` has three values) |
| gamepad hat bound as a key action | reported as soon as the key leaves rest | reported from the actuation point, as buttons are |
| gamepad binding of a key that has an advanced key on layer 0 | ignored | applies |
| dynamic keystroke release while its four-step queue is full | dropped, the key stays pressed | kept |
| rapid-trigger reference after a release | the deepest point until the next sample, so a key that turns down right at its release needs one sample's movement more to press again | the point where the key was released |
| stock feature commands with another report id or shorter than 7 command bytes | processed (short data read past its end) | stalled |
| boot keyboard report with more than six keys held | the seventh key is dropped, and because the report is rebuilt in scan order a key that scans earlier pushes out one still held | ErrorRollOver (`0x01`) in all six key slots, modifiers unchanged, while more than six keys are held (HID 1.11 appendix C) |
