# USB face and configuration channel

What a host sees when a keyboard runs openwoot: the USB device, its five interfaces and their reports (the "USB face", kept close to the stock firmware so that operating systems, games and the Wooting Analog SDK keep working), and the configuration channel that `openwoot` and other host software use to talk to the firmware.

| Part | Code |
| --- | --- |
| device and interfaces | `crates/openwoot/src/usb/` (`usb::build`, `usb::Face`, `usb::HostState`, the HID class request handler) |
| report descriptors and typed reports | `crates/openwoot/src/hid/` (`hid::DescriptorBuilder`, one module per report, re-exported by `hid`) |
| configuration channel, firmware side | `crates/openwoot/src/protocol/` (`protocol::Channel`, `protocol::Handler`, `protocol::UpdateMode`, the stock identification responder with `protocol::StockIdentity`); the stock feature-report queue `usb::FeatureRequests` |
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
| `SET_REPORT` output on interface 1 | the lock lights, published as `HostState::keyboard_leds` (bit 0 Num Lock, 1 Caps Lock, 2 Scroll Lock, 3 Compose, 4 Kana) |
| `SET_REPORT` feature on interface 2 | a stock feature command, queued in `HostState::feature_requests` for the [stock identification responder](#stock-identification-responder) |
| `SET_REPORT` output or input on interface 2 | accepted and ignored (stock output-report commands; none is supported) |
| `GET_REPORT` on interface 2 | the report id byte only, as stock; report id 0 stalls |
| string `0xee` in the XInput mode | Microsoft OS 1.0 string `MSFT100` with request code `0xb0` |
| vendor request `0xb0`, `wIndex` 4, in the XInput mode | 40-byte extended compat ID that gives interface 0 the compatible id `XUSB10`, so Windows binds its XInput driver without an INF file |

## Reports

Report descriptors are built at compile time by `hid::DescriptorBuilder` (a `const fn` item builder that encodes each item in its smallest size and fails compilation on unbalanced collections or overflow). Each report is a typed struct implementing `hid::InputReport` (`ID`, `LEN`, `to_bytes`), and `usb::write_report` sends it. The host tests parse every descriptor and check that each report's size equals its struct.

| Report | Bytes (little endian) |
| --- | --- |
| `hid::BootReport` (interface 1, boot) | modifiers (bit n = usage `0xe0` + n), 0, six key usages |
| `hid::NkroReport` (interface 1) | modifiers, then a 27-byte bitmap: usages `0x04`..`0x31`, `0x33`..`0x9b`, `0x9d`..`0xa4`, `0xb0`..`0xdd`, each group padded to a whole byte; one table (`hid::NKRO_KEY_GROUPS`) builds both the descriptor and the bit positions |
| keyboard output (interface 1) | one byte, five LED bits and three padding bits |
| `hid::SystemReport` (id 1) | id, control: 0 none, 1 power down, 2 sleep, 3 wake up (usages `0x81`..`0x83`) |
| `hid::ConsumerReport` (id 2) | id, three 16-bit consumer usages (1..`0x02a2`, 0 none) |
| `hid::MouseReport` (id 3) | id, five button bits and three padding bits, relative X and Y (-127..127) |
| `hid::GamepadReport` (interface 0, `Gamepad`) | 12 button bits and 4 padding bits, X, Y, Z, Rz, Rx, Ry (0..`0x7ffe`, centre `0x3fff`), throttle, elevator, accelerator, rudder, trigger (0..`0x7fff`), hat (1..8 clockwise from up, 0 centred) |
| `xinput::Report` (interface 0, `Xinput`) | `0x00`, `0x14`, buttons (16 bits), left and right trigger (8 bits each), left X, left Y, right X, right Y (signed 16 bits), six zero bytes |
| XInput output (`xinput::LedPattern`) | `0x01 0x03 <pattern>`: patterns 2..5 and 6..9 select player 1..4 |
| `hid::KeyStateReport` (interface 4) | 16 entries of 4 bytes: key position, usage code, then a 16-bit word: bit 0 pressed, bits 2..5 kind (0 keyboard, 3 consumer, 4 function, 5 gamepad, 6 advanced), bits 6..15 travel (0..1023); unused entries are 0 |

## Configuration channel

The channel uses interface 2 (usage page `0xff55`, usage 1) with the stock report ids and sizes: ids 1 to 6 carry 32, 62, 254, 510, 1022 and 2046 bytes (without the report id byte) as input and output reports; id 1 also declares the 7-byte stock feature report, which carries the stock commands of the [identification responder](#stock-identification-responder). The host writes output reports, which travel over the interrupt OUT endpoint `0x03` (hidapi `write`); the firmware answers with input reports on `0x83` (hidapi `read`). Every report ends with a short packet, so no zero-length packets are needed.

### Report envelope

```
report id | 0x4f 0x57 ("OW") | frame length (u16, little endian) | frame | zero padding
```

A frame goes into the smallest report that holds it (up to 2042 bytes in report 6). Stock traffic starts with `d1 da` or `ee` and never with `OW`, so the two cannot be confused (`openwoot_protocol::report::parse` returns `report::Contents::Foreign` for it).

### Frame

Each frame is one message in the format of `postcard-rpc` 0.12 (only its core: keys, header and standard errors; its transports are not used):

```
header byte | key (1, 2, 4 or 8 bytes) | sequence number (1, 2 or 4 bytes, little endian) | postcard body
```

The header byte holds the key size in bits 7..6 (`00` 1, `01` 2, `10` 4, `11` 8 bytes), the sequence number size in bits 5..4 (`00` 1, `01` 2, `10` 4 bytes) and the header version 0 in bits 3..0. A key is the hash of an endpoint's path and the schema of its request (for requests) or response (for responses), so a request whose type changed reaches no handler. The body is the request or response encoded with `postcard` 1.2.

The host sends requests with 8-byte keys and 2-byte sequence numbers (`openwoot_protocol::encode_request`). The firmware answers each request with one response that repeats the sequence number and uses a key of the request's key size: the endpoint's response key, or the error key (path `error`, type `postcard_rpc::standard_icd::WireError`, for example `UnknownKey` for a request it does not know). `openwoot_protocol::decode_response` returns the response, the device's error, or nothing for reports that answer something else. The firmware serves one request at a time; the host waits for the answer before it sends the next request.

### Endpoints

Every endpoint is declared once in `openwoot_protocol::endpoints` (one table, collected in `endpoints::LIST`). `Done` stands for `Result<(), Rejection>`; every other configuration response is `Result<T, Rejection>` with the type shown.

| Endpoint | Path | Request | Response |
| --- | --- | --- | --- |
| `GetVersion` | `openwoot/version` | `()` | `Version { protocol: u16, firmware: FirmwareVersion { major: u16, minor: u16, patch: u16 } }` |
| `EnterUpdateMode` | `openwoot/update-mode` | `()` | `()`; after sending it the firmware hands over to the factory bootloader (`UpdateMode::enter_update_mode` with option byte 0), which stays in update mode for one start |
| `GetCapabilities` | `openwoot/capabilities` | `()` | `config::Capabilities` |
| `GetDeviceSettings`, `SetDeviceSettings` | `openwoot/device`, `…/set` | `()`, `device::Settings` | `device::Settings`, `Done` |
| `GetLayout`, `SetLayout` | `openwoot/layout`, `…/set` | `()`, `device::Layout` | `device::Layout`, `Done` |
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
| `Save`, `Revert`, `FactoryReset` | `openwoot/save`, `openwoot/revert`, `openwoot/factory-reset` | `()` | `Done` |

`PROTOCOL_VERSION` is 1. The types behind a path never change: a changed request or response type gets a new path (`openwoot/profile/keymap/2`), so old and new hosts and firmware recognise each other's endpoints by key. A key is a hash of the path and the schema of the type (field names and type names included), so a test (`endpoint_keys_are_pinned_so_a_changed_type_needs_a_new_path`) pins the request and response key of every endpoint: a change of any type behind a path fails it. Capacities of `heapless::Vec` and `heapless::String` are not part of a schema; raising one is compatible for hosts that never send more than the old capacity, lowering one needs a new path.

### Configuration model

The configuration is a set of typed parts, each a `no_std` serde type in `openwoot_protocol`, sized by `heapless` capacities so that the largest value of every part fits one frame (tested). Storage keeps its own records of the same types; the C firmware's `settings.proto` was the inventory of what exists, not the format.

| Part | Type | Holds |
| --- | --- | --- |
| capabilities | `config::Capabilities` | key matrix rows and columns, number of profile slots, layouts the board can have, light-bar LEDs (0 = none), host signals, switch models, edition |
| device | `device::Settings` | USB mode (`Xinput`, `Gamepad`, `NoGamepad`), keyboard report (`NKeyRollover`, `Boot`), speed (`High`, `Full`), gamepad polling (4 ms, 1 ms, 125 µs); converted to `usb::Face` by `From`; takes effect at the next start |
| layout | `device::Layout` | `Ansi`, `Iso`, `Jis`; must be one of `Capabilities::layouts` |
| global | `global::Settings` | idle dimming (`lighting::Idle`: dim after, off after, off enabled; `None` = never), Num Lock indicator inverted, release-rebound filter |
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
| switches | `switches::Settings` | switch model and travel offset (0.05 mm) per key and a default; `GetSwitchModels` lists each model's travel and group |

Keys are addressed by `keys::Index`, `row * key_columns + column` of the board's key matrix; per-key lists hold exactly `key_rows * key_columns` entries. Distances (`keys::Distance`) are fractions of the full travel of the key's switch in 1/65535. Colours with transparency (`lighting::Color`) carry an alpha of 0 to 255 (255 opaque).

Actions (`keys::Action`): `None`, `Keyboard(usage)` (usages `0x04`..`0xdd` and the modifiers `0xe0`..`0xe7`), `Consumer(usage)` (`0x0001`..`0x02a2`), `System` (power down, sleep, wake up), `Mouse` (left, right, middle, back, forward), `Gamepad(control)`, `Layer(layer)` (momentary, not the base layer), `Function` (brightness up and down, profile, next profile, toggle mode profile, toggle key disable, layer lock).

Semantics:

- A `Set*` request applies at once: to the running keyboard when it addresses the active profile, otherwise to that profile. Nothing is stored until `Save`, which writes every change since the last save; `Revert` drops the unsaved changes; `FactoryReset` returns every setting and profile to the board's factory defaults (stored at once), the C firmware's factory reset.
- The firmware validates every request that carries a value against `Capabilities` before it reaches the keyboard (`config::Validate`) and answers `Err(Rejection::Invalid(..))` without changing anything. `Rejection` also reports a missing profile, a profile that cannot be deleted, the profile limit, a locked keyboard (switches), a storage error, and `Unsupported` for parts the keyboard does not have.
- Firmware side: `protocol::Handler` decodes, validates and hands each request to `protocol::Configuration`, a trait with one async method per part; every method answers `Unsupported` unless implemented, and `()` implements none (`Handler::new`). Storage, keys and lighting implement it together (`Handler::with_configuration`) without changes to the handler.

Differences from the C firmware's configuration: one profile namespace of `Capabilities::profiles` slots instead of built-in profiles 0..3 plus the host namespace `lp` 0..9 (the `lp` selection, its `al` files and the uninterpreted `lbm`/`glbm` files and profile icon string are not kept); key colours as 8-bit RGB per key instead of RGB565 in two halves; distances in one 16-bit encoding instead of the 8-bit and 14-bit encodings; mod-tap hold time in ms instead of 10 ms steps; light-bar meters and profile indicators draw on light-bar ranges only (C accepted key rectangles and drew nothing); effect kinds 12 and 13 (never drawn by C) and the legacy single-effect form do not exist; the legacy per-key rapid-trigger file and the rapid-trigger groups are one per-key override list; the switch profile is per key instead of groups of up to 10 keys. Validation replaces the C firmware's silent clamping and "known issue" behaviours: values C would clamp or divide by are refused.

### Stock identification responder

The official Wootility recognises keyboards by vendor id, product id and usage page `0xff55` and talks to them with stock feature commands. The responder answers the few it needs to recognise the keyboard and to install the stock firmware, so that returning to stock is the normal Wootility update (see [flashing](flashing.md)).

A stock feature command is a `SET_REPORT` of feature report 1 on interface 2 (`wValue` `0x0301`): `01 d1 da <command> <a0 a1 a2 a3>` (argument little endian; data without the report id byte is accepted too). The USB control handler checks and queues it in `HostState::feature_requests` (`usb::FeatureRequests`: four entries, at most three commands, as stock), and `protocol::Channel` answers it on `0x83` as an input report in the smallest report that holds it, zero padded:

```
report id | d1 da | command | status | payload length (u16, little endian) | payload
```

| Command | Argument | Answer (status `0x88` unless stated) |
| --- | --- | --- |
| 0, ping | `a0` | 1 byte: `a0` inverted |
| 1, version | none | `02 01 02`: version 2.1.2, see below |
| 2, reset to update mode | `a0`: option bits for the bootloader (bit 0 high speed, bit 1 ignore the key combination, bit 2 no lighting) | none; the channel calls `UpdateMode::enter_update_mode(a0)`, which writes `0x504` and `a0` to the backup registers and resets, as stock |
| 3, serial number, selector 0 | `arg` = 0 | 11 bytes: supplier (u16), year, week, product (u16), revision (u16), product id (u16, at most `0xfffe`), stage; all zero without a serial number |
| 3, selector 1 | `arg` = 1 | 12 bytes: the 96-bit unique id of the microcontroller (`StockIdentity::mcu_id`, read by `hal::flash::unique_id`) |
| 3, selector 2 | `arg` = 2 | the protobuf message `SerialNumber` (fields 1 to 7 always, 9 edition, 10 PCB design, 11 minor revision when present), encoded as the stock firmware encodes it; without a serial number status `0xff` with the 16-bit error -2 (`fe ff`, no device information) |
| 3, selector 3 | `arg` = 3 | 10 bytes: manufacturer and device id of the settings flash (command `0x90`) and its 8-byte unique id (command `0x4b`), `StockIdentity::flash_ids`; zero if the chip does not answer |
| 3, other selectors | | status `0xff`, no payload |
| 19, device settings | none | 7 bytes: byte 0 bit 3 full speed forced (`usb::Speed::Full`), bit 4 boot keyboard report; bytes 1 to 3 zero; byte 4 `0x11` (flash read-out allowed); byte 5 the physical layout (`StockIdentity::layout`: Wooting 80HE device-information record, Wooting 60HE+ bit 0 of the user option byte); byte 6 bits 2..1 USB mode (`usb::Mode`: 0 XInput, 1 gamepad, 2 none) and bits 4..3 gamepad polling (0 4 ms, 1 1 ms, 2 125 µs) |
| every other command | | no payload; status `0x66` (unknown) for numbers below 38 and above 88, `0xaa` (unsupported) for 38 to 88, the stock rule for commands without a handler |

A request with another magic gets status `0xff` (under the magic `d1 da`), and a fourth waiting command status `0x77`, both at once as stock does.

The serial number comes from `StockIdentity::serial`, the serial text the board reads from the serial page of the microcontroller (for example `A01B2401W01T00100S01H00300`, a made-up serial: supplier 01, year 24, week 01, product 01, PCB design 00, revision 1, minor revision 00, edition 01, stage H, product id 00300). The official Wootility identifies a keyboard by these fields and finds it again by them after the update, so they must equal what the stock firmware reports.

Version 2.1.2 is a factory version for the official Wootility: it asks for the update to the current stock firmware before anything else, reads no profiles first (openwoot answers none), and resets the stock settings after the update. The C reference reports 2.14.1, with which Wootility would treat the keyboard as up to date and read its profiles.

Stock output-report commands (`d1 da` in an output report on `0x03` or through `SET_REPORT`) get no answer, as stock does for unknown ones; `OW` frames on the same reports work as described above.

## Deviations from the stock face

| Topic | Stock firmware | openwoot |
| --- | --- | --- |
| `DEVICE_QUALIFIER` | stalled | answered on the high-speed Wooting 80HE (embassy-usb); stalled on the full-speed-only Wooting 60HE+ by its USB driver, as stock |
| BOS | stalled | answered with an empty BOS (only asked for by hosts when `bcdUSB` is 2.01 or higher) |
| `GET_STATUS` on an interface | stalled | answered with 0 |
| `SET_INTERFACE` with an alternate setting above 0, `SET_CONFIGURATION` above 1, `SET_ADDRESS` 0 | acknowledged | stalled |
| interface-0 `bInterval` when the Wooting 80HE falls back to full speed | switched to the full-speed value | built once for the configured speed, so a high-speed build on a full-speed hub polls interface 0 at the high-speed value read as frames (4 ms instead of 1 ms at the default rate) |
| resume | reported at the first start of frame after the wake-up interrupt | reported at the wake-up interrupt |
| configuration channel OUT endpoint | not re-armed until the command is processed | one packet of the next transfer may arrive before the current one is processed (the channel's request/response pairing is the flow control) |
| `SET_REPORT` on interface 2 (feature or output report through the control endpoint) | processed as a stock command, up to 64 (feature) or 2048 (output) bytes | feature reports go to the stock identification responder; output reports are accepted and ignored, so `OW` frames travel only over the interrupt OUT endpoint |
| `GET_REPORT` on interfaces other than 2 | answered with the report id byte only | stalled |
| stock feature commands | 65 commands on the Wooting 80HE, 59 on the Wooting 60HE+ | commands 0, 1, 2, 3 and 19 (version 2.1.2 instead of 2.14.1); the others are answered as unknown or unsupported |
| control transfers longer than 128 bytes | accepted up to 2048 bytes | stalled before the data stage |
| report descriptors | stock byte arrays | same fields (checked against the C build field by field); byte encodings differ where items are ordered differently or encoded in their smallest size, for example the boot keyboard's logical maximum 221 is `26 dd 00` where stock writes `25 dd` (which reads as -35) |
| gamepad hat switch | Input flags `0x02` | `0x42` (Null State), so the centred value 0 is declared as "no direction" |
| interface 3 reports | always 7 bytes (id and 6 data bytes) | each report at its own length: 2 bytes (system), 7 (consumer), 4 (mouse) |
| USB mode 3 | interface 0 answers with the keyboard report descriptor | not representable (`usb::Mode` has three values) |
