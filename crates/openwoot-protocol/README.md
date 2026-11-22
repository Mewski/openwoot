# openwoot-protocol

The configuration channel between the openwoot firmware and host software, `no_std`.

- `endpoints`: every endpoint in one `postcard-rpc` 0.12 table (`endpoints::LIST`), for example
  `GetVersion` (request `()`, response `Version`) and `EnterUpdateMode` (request `()`, response
  `()`, then the firmware resets into update mode).
- Messages: a `postcard-rpc` header (key, sequence number) and a `postcard` body. Host side:
  `encode_request` and `decode_response`; firmware side: `decode_message`, `Message::reply_header`
  and `encode_message`.
- `report`: each message travels in one HID report of interface 2 (usage page `0xff55`) as `"OW"`,
  the frame length (`u16`, little endian) and the frame, zero padded to the smallest report
  (ids 1 to 6: 32, 62, 254, 510, 1022, 2046 bytes, `report::SIZES`) that holds it.
- `stock`: the stock feature-command framing (`d1 da`, command numbers, status values, request and
  reply encoding) shared by the firmware's identification responder and the host tool.
- The configuration model, one namespace per part: `config` (capabilities, validation,
  rejections), `device`, `global`, `profile`, `keys`, `advanced`, `gamepad`, `lighting`,
  `lightbar`, `switches`.

The full description is in `docs/protocol.md`.
