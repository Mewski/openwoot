# Extending the keys

How a key press becomes a report, and where a new key feature goes. The C firmware on `main` is the behavioural reference; its `docs/extending-keys.md` describes the same pipeline in more depth.

## Pipeline

| Step | Where | Runs |
| --- | --- | --- |
| scan cycle | `scan::Shared`, conversion interrupt | samples every sensor, turns the value into travel 0..4095 and queues an event for every key that is not at rest, once more when it returns to rest, and while `Sensor::hold_events` is set |
| key task | `runtime::KeyTask` on the key executor | after every cycle: takes configuration changes (`keys::Shared`), refreshes each key's switch facts, runs one pass, publishes the reports (`runtime::KeyReports`), forwards key events and notices to lighting, sets `hold_events`; it awaits nothing but the scan cycle (hand-offs to storage, lighting and USB never wait), and a cycle that does not end within 50 ms is restarted (`scan::Shared::recover`, `scan::Converter::recover`) |
| pass | `keys::Keys::pass` | per event: binding layer, press decision or advanced-key input stage, press/release edge, output (advanced-key output stage or keymap action); then the two-key resolvers, the gamepad report and the release of special actions |
| sink | `keys::Keys::apply_action` | one action into the key-state report and the keyboard (6KRO or NKRO by the descriptor), system, consumer, mouse report, a special action (`keys::Functions`) or the gamepad input |
| reports | `runtime::Reports` on the USB executor | writes every report that changed since it was last sent |

A key press reaches its report at the end of the scan cycle that sampled it (one cycle of latency); `tests/it/keys/pipeline.rs` checks this through a simulated converter.

Travel is the unit of every threshold inside `keys`: the configuration's distances (`openwoot_protocol::keys::Distance`, a fraction of the full travel) become travel in `keys::travel_of` when the key task applies a profile, and minimum distances (0.1 mm of each key's switch, `keys::Switch`) are applied there.

## Where a feature goes

- A new press decision: `keys/actuation.rs` (`Actuation::update`), selected per key in `keys/engine.rs` (`trigger`).
- A new advanced key: a variant of `openwoot_protocol::advanced::Behaviour` (new endpoint path, see `docs/protocol.md`), its engine form in `keys::Behaviour`, a state type in `keys/advanced/` with an input stage (`input`, fed with the key's travel or press state) and an output stage (`run`, returning the actions to apply), and the two `match` arms in `Keys::advanced_input` and `Keys::run_advanced`.
- A new key action: a variant of `openwoot_protocol::keys::Action`, its report in `Keys::apply_action` and its key-state code in `keys/output.rs`.
- A new special (Fn) action: a variant of `keys::Special` with its code, and its effect in `layout::Standard` (layers, profiles, key flags, lighting notices through `keys::Effects`).
- Something lighting must show: a `keys::Notice` variant, applied in `runtime::KeyNotices::apply`.
- Factory keymaps: `boards/<board>/src/keys.rs` (`layout::DefaultKeymap` with the layers and the layer-0 patches of each layout variant).

## Settings and storage

`profile::Model` holds the active profile: its key part (settings, keymaps, overrides, advanced keys, gamepad mapping and settings) in `keys::Shared`, its lighting part in `profile::Shared`. Each key part is its own record (`keys/profile.rs`, items `0x10`..`0x15` per profile, the active profile as global item `0x10`). `profile::Configurator` answers the configuration channel: a `Set*` applies at once, `Save` stores the changed parts of the active profile, `Revert` reloads them. The key task applies a changed part between two scan cycles, reading it through `keys::ProfileSource` one entry (one key's action, override or binding, one advanced key) per critical section, and the model resets a profile's keymaps one key per critical section, so a profile switch holds each critical section for one entry or one copy of a loaded part, never for the conversion of a whole profile; `Parts::PROFILES` carries a changed profile count, and the release-rebound filter of the global settings reaches the scanner through `keys::Shared::rebound_filter`. Key reports on and off, gamepad detection and the XInput player are host-link state in `usb::HostState`, which the key task copies into `keys::Keys` every cycle.

## Tests

Unit tests next to each state machine; `tests/it/keys/` drives the engine pass by pass and the whole pipeline through a simulated scan; the private golden vectors of the C build cover the press decision, the action sink, every advanced key and the gamepad math.
