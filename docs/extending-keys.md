# Extending the keys

How a key press becomes a report, and where a new key feature goes.

## Pipeline

| Step | Where | Runs |
| --- | --- | --- |
| scan cycle | `scan::Shared`, conversion interrupt | samples every sensor, turns the value into travel 0..4095 and queues an event for every key that is not at rest, once more when it returns to rest, and while `Sensor::hold_events` is set |
| key task | `runtime::KeyTask` on the key executor | after every cycle (`KeyTask::pass`): takes configuration changes (`keys::Shared`), refreshes the keys' switch facts when the switches changed, runs one pass, sets `hold_events` for the keys the pass touched (`Keys::take_hold_changes`), and fills a pending sensor readout (`GetSensorValues`); then (`KeyTask::deliver`) publishes the reports (`runtime::KeyReports`) and forwards key events and notices to lighting, in the single-input scan while the next cycle already runs; it awaits nothing but the scan cycle, an LED transfer it lets out between two cycles and, in the six-input scan, 100 µs of idle CPU after every millisecond (hand-offs to storage, lighting and USB never wait), and a cycle that does not end within 50 ms is restarted (`scan::Shared::recover`, `scan::Converter::recover`) |
| pass | `keys::Keys::pass` | per event: binding layer, press decision or advanced-key input stage (a press that decides a waiting mod-tap is decided again on the layer it ends up on), press/release edge, output (advanced-key output stage or keymap action, then the key's gamepad binding); then the two-key resolvers, the gamepad report and the release of special actions; while key reports are off every key takes the plain press decision and advanced keys rest |
| sink | `keys::Keys::apply_action` | one action into the key-state report and the keyboard (6KRO in the boot format of the descriptor or of the host's boot protocol, NKRO otherwise), system, consumer, mouse report, a special action (`keys::Functions`) or the gamepad input |
| macros | `keys::Keys::play`, `macros::Player` | after the pass: a press of a key bound to `Action::Macro` asks thread mode to load that macro into the playback buffer (`macros::Shared` in `keys::Shared`); once it is loaded, the player adds what the macro holds (keys, buttons, a text keystroke, one movement) to the output, one change per pass and only after the reports forwarder took the report of the previous change (`runtime::KeyReports::drained`); `KeyTask::pass` stops it on suspend and bus reset, the pass on a layer key press, a profile switch and key reports going off ([protocol](protocol.md#configuration-model)) |
| reports | `runtime::Reports` on the device executor | writes every report that changed since it was last sent |

A key press reaches its report at the end of the scan cycle that sampled it (one cycle of latency); `tests/it/keys/pipeline.rs` checks this through a simulated converter.

Travel is the unit of every threshold inside `keys`: the configuration's distances (`openwoot_protocol::keys::Distance`, a fraction of the full travel) become travel in `keys::travel_of` when the key task applies a profile, and minimum distances (0.1 mm of each key's switch, `keys::Switch`) are applied there.

## Where a feature goes

- A new press decision: `keys/actuation.rs` (`Actuation::update`), selected per key in `keys/engine.rs` (`trigger`).
- A new advanced key: a variant of `openwoot_protocol::advanced::Behaviour` (new endpoint path, see `docs/protocol.md`), its engine form in `keys::Behaviour`, its kind, actions and initial state (`Behaviour::kind`, `actions`, `initial_state`) and its conversion (`Entry::from_settings`), a state type in `keys/advanced/` with an input stage (`input`, fed with the key's travel or press state) and an output stage (`run`, returning the actions to apply), and the two `match` arms in `Keys::advanced_input` and `Keys::run_advanced`, which fall through silently when an arm is missing. A two-key behaviour instead keeps both keys in one `PairState` (`capture` in the input stage, `record` in the output stage) and decides in `Keys::resolve_pairs` (`resolve_depth`, `resolve_socd`) after every event of the pass.
- A new key action: a variant of `openwoot_protocol::keys::Action`, its report in `Keys::apply_action` and its key-state code in `keys/output.rs`.
- A new special (Fn) action: a variant of `keys::Special` with its code in both `Special::code` and `Special::from_code` (below `SPECIALS` in `keys/special.rs`, which sizes the request table; a code beyond it is never held or released), and its effect in `layout::Standard` (layers, profiles, key flags, lighting notices through `keys::Effects`).
- Something lighting must show: a `keys::Notice` variant, applied in `runtime::KeyNotices::apply`.
- Factory keymaps: `boards/<board>/src/keys.rs` (`layout::DefaultKeymap` with the layers and the per-layer patches of each layout variant of each layout variant).

## Settings and storage

`profile::Model` holds the active profile: its key part (settings, keymaps, overrides, advanced keys, gamepad mapping and settings) in `keys::Shared`, its lighting part in `profile::Shared`. Each key part is its own record (`keys/profile.rs`, items `0x10`..`0x15` per profile, the active profile as global item `0x10`). `profile::Configurator` answers the configuration channel: a `Set*` applies at once, `Save` stores the changed parts of the active profile, `Revert` reloads them. The key task applies a changed part between two scan cycles, reading it through `keys::ProfileSource` one entry (one key's action, override or binding, one advanced key) per critical section, the model resets a profile's keymaps one key per critical section, and `Save` encodes a key part the same way (`profile/key_part.rs`: one sequence length or one entry copied per critical section, encoded outside it), so a profile switch or a save holds each critical section for one entry or one copy of a loaded part, never for the conversion of a whole profile; `Parts::PROFILES` carries a changed profile count, and the release-rebound filter of the global settings reaches the scanner through `keys::Shared::rebound_filter`. Key reports on and off, gamepad detection and the XInput player are host-link state in `usb::HostState`, which the key task copies into `keys::Keys` every cycle.

## Tests

Unit tests next to each state machine; `tests/it/keys/` drives the engine pass by pass and the whole pipeline through a simulated scan.
