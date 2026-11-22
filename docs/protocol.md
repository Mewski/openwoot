# USB face and configuration channel

What a host sees when a keyboard runs openwoot: the USB device, its five interfaces and their reports (the "USB face", kept close to the stock firmware so that operating systems, games and the Wooting Analog SDK keep working), and the configuration channel that `openwoot` and other host software use to talk to the firmware.

| Part | Code |
| --- | --- |
| device and interfaces | `crates/openwoot/src/usb/` (`usb::build`, `usb::Face`, `usb::Control`) |
| report descriptors and typed reports | `crates/openwoot/src/hid/` (`hid::DescriptorBuilder`, one module per report, re-exported by `hid`) |
| configuration channel, firmware side | `crates/openwoot/src/protocol/` (`protocol::Channel`, `protocol::Handler`) |
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
| `hid::XinputReport` (interface 0, `Xinput`) | `0x00`, `0x14`, buttons (16 bits), left and right trigger (8 bits each), left X, left Y, right X, right Y (signed 16 bits), six zero bytes |
| XInput output (`usb::XinputLedPattern`) | `0x01 0x03 <pattern>`: patterns 2..5 and 6..9 select player 1..4 |
| `hid::KeyStateReport` (interface 4) | 16 entries of 4 bytes: key position, usage code, then a 16-bit word: bit 0 pressed, bits 2..5 kind (0 keyboard, 3 consumer, 4 function, 5 gamepad, 6 advanced), bits 6..15 travel (0..1023); unused entries are 0 |

## Configuration channel

The channel uses interface 2 (usage page `0xff55`, usage 1) with the stock report ids and sizes: ids 1 to 6 carry 32, 62, 254, 510, 1022 and 2046 bytes (without the report id byte) as input and output reports; id 1 also declares a 7-byte feature report, kept for the planned identification responder. The host writes output reports, which travel over the interrupt OUT endpoint `0x03` (hidapi `write`); the firmware answers with input reports on `0x83` (hidapi `read`). Every report ends with a short packet, so no zero-length packets are needed.

### Report envelope

```
report id | 0x4f 0x57 ("OW") | frame length (u16, little endian) | frame | zero padding
```

A frame goes into the smallest report that holds it (up to 2042 bytes in report 6). Stock traffic starts with `d1 da` or `ee` and never with `OW`, so the two cannot be confused (`openwoot_protocol::parse_report` returns `Contents::Foreign` for it).

### Frame

Each frame is one message in the format of `postcard-rpc` 0.12 (only its core: keys, header and standard errors; its transports are not used):

```
header byte | key (1, 2, 4 or 8 bytes) | sequence number (1, 2 or 4 bytes, little endian) | postcard body
```

The header byte holds the key size in bits 7..6 (`00` 1, `01` 2, `10` 4, `11` 8 bytes), the sequence number size in bits 5..4 (`00` 1, `01` 2, `10` 4 bytes) and the header version 0 in bits 3..0. A key is the hash of an endpoint's path and the schema of its request (for requests) or response (for responses), so a request whose type changed reaches no handler. The body is the request or response encoded with `postcard` 1.2.

The host sends requests with 8-byte keys and 2-byte sequence numbers (`openwoot_protocol::encode_request`). The firmware answers each request with one response that repeats the sequence number and uses a key of the request's key size: the endpoint's response key, or the error key (path `error`, type `postcard_rpc::standard_icd::WireError`, for example `UnknownKey` for a request it does not know). `openwoot_protocol::decode_response` returns the response, the device's error, or nothing for reports that answer something else. The firmware serves one request at a time; the host waits for the answer before it sends the next request.

### Endpoints

| Endpoint | Path | Request | Response |
| --- | --- | --- | --- |
| `GetVersion` | `openwoot/version` | `()` | `Version { protocol: u16, firmware: FirmwareVersion { major: u16, minor: u16, patch: u16 } }` |

`PROTOCOL_VERSION` is 1. The types behind a path never change: a changed request or response type gets a new path (`openwoot/version/2`), so old and new hosts and firmware recognise each other's endpoints by key.

### Planned: stock identification responder

The official Wootility finds keyboards by vendor id, product id and usage page `0xff55` and sends stock commands (`d1 da ...`, also as feature report 1). A minimal responder for the stock identification commands (ping, version, serial number, reset into update mode) will let it recognise the keyboard and offer the stock firmware. It is not built yet: its answers are fixed after one recorded Wootility session. Until then stock requests get no answer, and stock feature and output reports sent through the control endpoint are stalled.

## Deviations from the stock face

| Topic | Stock firmware | openwoot |
| --- | --- | --- |
| `DEVICE_QUALIFIER` | stalled | answered (embassy-usb); correct for the high-speed Wooting 80HE, a small embassy-usb change is pending for the full-speed-only Wooting 60HE+ |
| BOS | stalled | answered with an empty BOS (only asked for by hosts when `bcdUSB` is 2.01 or higher) |
| `GET_STATUS` on an interface | stalled | answered with 0 |
| `SET_INTERFACE` with an alternate setting above 0, `SET_CONFIGURATION` above 1, `SET_ADDRESS` 0 | acknowledged | stalled |
| interface-0 `bInterval` when the Wooting 80HE falls back to full speed | switched to the full-speed value | built once for the configured speed, so a high-speed build on a full-speed hub polls interface 0 at the high-speed value read as frames (4 ms instead of 1 ms at the default rate) |
| resume | reported at the first start of frame after the wake-up interrupt | reported at the wake-up interrupt |
| configuration channel OUT endpoint | not re-armed until the command is processed | one packet of the next transfer may arrive before the current one is processed (the channel's request/response pairing is the flow control) |
| `SET_REPORT` on interface 2 (feature or output report through the control endpoint) | processed as a stock command, up to 64 (feature) or 2048 (output) bytes | stalled; the channel reads the interrupt OUT endpoint only (until the identification responder) |
| `GET_REPORT` | answered with the report id byte only | stalled |
| control transfers longer than 128 bytes | accepted up to 2048 bytes | stalled before the data stage |
| report descriptors | stock byte arrays | same fields (checked against the C build field by field); byte encodings differ where items are ordered differently or encoded in their smallest size, for example the boot keyboard's logical maximum 221 is `26 dd 00` where stock writes `25 dd` (which reads as -35) |
| gamepad hat switch | Input flags `0x02` | `0x42` (Null State), so the centred value 0 is declared as "no direction" |
| interface 3 reports | always 7 bytes (id and 6 data bytes) | each report at its own length: 2 bytes (system), 7 (consumer), 4 (mouse) |
| USB mode 3 | interface 0 answers with the keyboard report descriptor | not representable (`usb::Mode` has three values) |
