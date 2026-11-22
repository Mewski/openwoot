# openwoot-protocol

Messages between the openwoot firmware and host software, `no_std`, serialized with `serde`.

Today: `Request::GetVersion` and `Response::Version { protocol, firmware }`, `PROTOCOL_VERSION`,
and the framing constants `MAGIC` (`"OW"`, the first bytes of every frame) and `MAX_FRAME_LEN`.
