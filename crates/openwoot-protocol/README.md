# openwoot-protocol

The configuration channel between the openwoot firmware and host software, `no_std`.

- Endpoints in the style of `postcard-rpc` 0.12: `GetVersion` (request `()`, response `Version`).
- Messages: a `postcard-rpc` header (key, sequence number) and a `postcard` body.
- Reports: each message travels in one HID report of interface 2 (usage page `0xff55`) as
  `"OW"`, the frame length (`u16`, little endian) and the frame, zero padded to the smallest
  report (ids 1 to 6: 32, 62, 254, 510, 1022, 2046 bytes) that holds it.
- Host side: `encode_request` and `decode_response`; firmware side: `decode_message`,
  `Message::reply_header` and `encode_message`.

The full description is in `docs/protocol.md`.
