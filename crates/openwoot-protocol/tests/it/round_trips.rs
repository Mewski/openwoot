use openwoot_protocol::{
    advanced::{self, Behaviour, DepthPriority, Interrupt, ModTap, Resolution, Socd},
    configuration::{MAX_KEYS, Rejection},
    device::{self, Layout},
    endpoints::{
        GetAdvancedKeys, GetCapabilities, GetKeyColors, GetKeymap, GetLightbar, GetSensorValues,
        GetSwitches, SetAdvancedKeys, SetDeviceSettings, SetGamepadMapping, SetGamepadSettings,
        SetGlobalLightbar, SetGlobalSettings, SetHostColors, SetHostKeyColor, SetHostSignals,
        SetKeyColors, SetKeyOverrides, SetKeySettings, SetKeymap, SetLayout, SetLightbar,
        SetLighting, SetProfileInfo, SetSwitches,
    },
    gamepad::{self, Control},
    global,
    heapless::String,
    host_lighting::KeyColor,
    keys::{self, Action, Function, Index, Layer, MouseButton, Position, SystemControl},
    lightbar::{self, HostSignal},
    lighting::{Idle, Rgb},
    profile::{self, Change, LayerChange, LayerRef, Slot},
    sensors::Reading,
    switches::{self, Switch},
};

use crate::support::{
    DONE, capabilities, filled, gamepad_settings, key_count, key_settings, lightbar_settings,
    lighting_settings, opaque, round_trip, switches, white, widest_action, widest_advanced_key,
    widest_lightbar_entry, widest_override,
};

#[test]
fn keymaps_and_key_settings_survive_a_round_trip() {
    let mut keymap: keys::Keymap = filled(Action::None, key_count());
    keymap[0] = Action::Keyboard(0x29);
    keymap[1] = Action::Mouse(MouseButton::Back);
    keymap[2] = Action::System(SystemControl::Sleep);
    keymap[3] = Action::Gamepad(Control::LeftTrigger);
    keymap[4] = Action::Layer(Layer::Two);
    keymap[5] = Action::Function(Function::Profile(Slot::new(3)));
    let layer = LayerRef {
        profile: Slot::new(1),
        layer: Layer::One,
    };
    round_trip!(
        SetKeymap,
        LayerChange {
            profile: layer.profile,
            layer: layer.layer,
            value: keymap.clone(),
        },
        DONE,
    );
    round_trip!(GetKeymap, layer, Ok(keymap));
    round_trip!(
        SetKeySettings,
        Change {
            profile: Slot::new(0),
            value: key_settings(),
        },
        DONE,
    );
    round_trip!(
        SetKeyOverrides,
        Change {
            profile: Slot::new(0),
            value: filled(widest_override(), key_count()),
        },
        Err(Rejection::StorageInaccessible),
    );
}

#[test]
fn advanced_keys_and_gamepad_settings_survive_a_round_trip() {
    let mod_tap = Behaviour::ModTap(ModTap {
        tap: Action::Keyboard(0x29),
        hold: Action::Keyboard(0xE0),
        hold_time_ms: ModTap::DEFAULT_HOLD_TIME_MS,
        interrupt: Interrupt::Hold,
    });
    let depth = Behaviour::DepthPriority(DepthPriority {
        other: Index::new(23),
        both_bottomed_out: Resolution::LastPressed,
    });
    let socd = Behaviour::Socd(Socd {
        other: Index::new(25),
        resolution: Resolution::Neither,
        both_when_bottomed_out: true,
    });
    let toggle = Behaviour::Toggle(Action::Keyboard(0x39));
    let advanced: advanced::Keys =
        core::iter::once(widest_advanced_key())
            .chain([(20, mod_tap), (21, toggle), (22, depth), (24, socd)].map(
                |(key, behaviour)| advanced::Key {
                    key: Index::new(key),
                    layer: Layer::Base,
                    behaviour,
                },
            ))
            .collect();
    round_trip!(
        SetAdvancedKeys,
        Change {
            profile: Slot::new(2),
            value: advanced.clone(),
        },
        DONE,
    );
    round_trip!(GetAdvancedKeys, Slot::new(2), Ok(advanced));
    let mut bindings = filled(None, key_count());
    bindings[7] = Some(Control::HatUp);
    round_trip!(
        SetGamepadMapping,
        Change {
            profile: Slot::new(0),
            value: gamepad::Mapping {
                bindings,
                exclusive: true,
            },
        },
        DONE,
    );
    round_trip!(
        SetGamepadSettings,
        Change {
            profile: Slot::new(0),
            value: gamepad_settings(),
        },
        DONE,
    );
}

#[test]
fn lighting_and_light_bar_survive_a_round_trip() {
    round_trip!(
        SetLighting,
        Change {
            profile: Slot::new(3),
            value: lighting_settings(),
        },
        Err(Rejection::ProfileMissing(3)),
    );
    round_trip!(
        SetKeyColors,
        LayerChange {
            profile: Slot::new(0),
            layer: Layer::Base,
            value: Some(filled(white(), key_count())),
        },
        DONE,
    );
    round_trip!(
        GetKeyColors,
        LayerRef {
            profile: Slot::new(0),
            layer: Layer::Three,
        },
        Ok(None),
    );
    round_trip!(
        SetLightbar,
        Change {
            profile: Slot::new(0),
            value: lightbar_settings(),
        },
        DONE,
    );
    round_trip!(GetLightbar, Slot::new(0), Ok(lightbar_settings()));
    round_trip!(SetGlobalLightbar, lightbar_settings().entries, DONE);
    let signals: lightbar::HostSignals = [HostSignal {
        id: 0xC0FF_EE00,
        value: 0.5,
    }]
    .into_iter()
    .collect();
    round_trip!(SetHostSignals, signals, DONE);
}

#[test]
fn keyboard_wide_settings_survive_a_round_trip() {
    round_trip!(SetSwitches, switches(), Err(Rejection::Locked));
    round_trip!(GetSwitches, (), Ok(switches()));
    round_trip!(
        SetDeviceSettings,
        device::Settings {
            mode: device::Mode::Xinput,
            keyboard: device::KeyboardFormat::Boot,
            speed: device::Speed::Full,
            gamepad_polling: device::PollingRate::Every125Microseconds,
        },
        DONE,
    );
    round_trip!(SetLayout, Layout::Jis, DONE);
    round_trip!(
        SetGlobalSettings,
        global::Settings {
            idle: Some(Idle {
                dim_after_minutes: 5,
                off_after_minutes: 10,
                off_enabled: true,
            }),
            num_lock_inverted: true,
            release_rebound_filter: false,
        },
        DONE,
    );
    round_trip!(
        SetProfileInfo,
        Change {
            profile: Slot::new(1),
            value: profile::Info {
                name: String::try_from("Rapid Profile").unwrap(),
                color: Rgb {
                    red: 255,
                    green: 40,
                    blue: 40,
                },
            },
        },
        DONE,
    );
    round_trip!(GetCapabilities, (), Ok(capabilities()));
}

#[test]
fn the_largest_key_configuration_fits_one_frame() {
    let profile = Slot::new(u8::MAX);
    round_trip!(
        SetKeymap,
        LayerChange {
            profile,
            layer: Layer::Three,
            value: filled(widest_action(), MAX_KEYS),
        },
        DONE,
    );
    round_trip!(
        SetKeyOverrides,
        Change {
            profile,
            value: filled(widest_override(), MAX_KEYS),
        },
        DONE,
    );
    round_trip!(
        SetAdvancedKeys,
        Change {
            profile,
            value: filled(widest_advanced_key(), advanced::MAX_ENTRIES),
        },
        DONE,
    );
    round_trip!(
        SetGamepadMapping,
        Change {
            profile,
            value: gamepad::Mapping {
                bindings: filled(Some(Control::HatRight), MAX_KEYS),
                exclusive: true,
            },
        },
        DONE,
    );
    let widest_switch = Some(Switch {
        model: u8::MAX,
        travel_offset: u8::MAX,
    });
    round_trip!(
        SetSwitches,
        switches::Settings {
            default: widest_switch,
            keys: filled(widest_switch, MAX_KEYS),
        },
        DONE,
    );
}

#[test]
fn the_largest_lighting_configuration_fits_one_frame() {
    let profile = Slot::new(u8::MAX);
    round_trip!(
        SetKeyColors,
        LayerChange {
            profile,
            layer: Layer::Three,
            value: Some(filled(white(), MAX_KEYS)),
        },
        DONE,
    );
    let entries: lightbar::Entries = filled(widest_lightbar_entry(), lightbar::MAX_ENTRIES);
    round_trip!(
        SetLightbar,
        Change {
            profile,
            value: lightbar::Settings {
                enabled: true,
                entries: entries.clone(),
                use_global: true,
                brightness: Some(u8::MAX),
            },
        },
        DONE,
    );
    round_trip!(SetGlobalLightbar, entries, DONE);
    let signals: lightbar::HostSignals = filled(
        HostSignal {
            id: u32::MAX,
            value: f32::MAX,
        },
        lightbar::MAX_HOST_SIGNALS,
    );
    round_trip!(SetHostSignals, signals, DONE);
    let mut name = String::new();
    while name.push('\u{20AC}').is_ok() {}
    round_trip!(
        SetProfileInfo,
        Change {
            profile,
            value: profile::Info {
                name,
                color: Rgb::default(),
            },
        },
        DONE,
    );
}

#[test]
fn host_lighting_and_full_sensor_readouts_fit_one_frame() {
    round_trip!(SetHostColors, filled(white(), MAX_KEYS), DONE);
    round_trip!(
        SetHostKeyColor,
        KeyColor {
            key: Position {
                row: u8::MAX,
                column: u8::MAX,
            },
            color: white(),
        },
        DONE,
    );
    for reading in [
        Reading::Travel,
        Reading::Value,
        Reading::RestLevel,
        Reading::BottomLevel,
    ] {
        round_trip!(
            GetSensorValues,
            reading,
            Ok::<_, Rejection>(filled(Some(u16::MAX), MAX_KEYS)),
        );
    }
}

#[test]
fn the_largest_diagnostics_report_fits_one_frame() {
    use openwoot_protocol::{
        diagnostics::{
            Bus, BusError, BusErrors, Keys, Lighting, Profile, Report, Scan, Speed, Stack, Storage,
            Usb,
        },
        endpoints::GetDiagnostics,
    };

    let bus = Bus {
        completed: u32::MAX,
        errors: BusErrors {
            timeout: u32::MAX,
            bus: u32::MAX,
            arbitration_loss: u32::MAX,
            no_acknowledge: u32::MAX,
            overrun: u32::MAX,
            dma: u32::MAX,
            other: u32::MAX,
        },
        last_error: Some(BusError::ArbitrationLoss),
    };
    round_trip!(
        GetDiagnostics,
        (),
        Report {
            scan: Scan {
                cycles: u32::MAX,
                recoveries: u32::MAX,
                overruns: Some(u32::MAX),
            },
            keys: Keys {
                passes: u32::MAX,
                reports: u32::MAX,
            },
            profile: Profile {
                active: Some(u8::MAX),
                requests: u32::MAX,
                switches: u32::MAX,
            },
            lighting: Lighting {
                initialized: true,
                active: true,
                frames: u32::MAX,
                page_writes: u32::MAX,
                failed_page_writes: u32::MAX,
            },
            led_bus: bus,
            storage: Storage {
                loads: u32::MAX,
                stores: u32::MAX,
                failures: u32::MAX,
            },
            flash_bus: Bus {
                last_error: Some(BusError::Dma),
                ..bus
            },
            usb: Usb {
                configured: true,
                suspended: true,
                speed: Some(Speed::High),
                resets: u32::MAX,
            },
            stack: Stack {
                size: u32::MAX,
                peak: u32::MAX,
            },
        },
    );
}

#[test]
fn profile_management_and_session_endpoints_survive_a_round_trip() {
    use openwoot_protocol::endpoints::{
        ActivateProfile, CreateProfile, DeleteProfile, EnterUpdateMode, FactoryReset,
        GetProfileInfo, GetProfileRevision, GetProfiles, Restart, Revert, Save,
    };

    round_trip!(
        GetProfiles,
        (),
        Ok(profile::Summary {
            count: 3,
            active: Slot::new(2),
        }),
    );
    round_trip!(CreateProfile, (), Ok(Slot::new(3)));
    round_trip!(CreateProfile, (), Err(Rejection::ProfileLimitReached));
    round_trip!(
        DeleteProfile,
        Slot::new(0),
        Err(Rejection::ProfileProtected(0))
    );
    round_trip!(ActivateProfile, Slot::new(1), DONE);
    round_trip!(
        GetProfileInfo,
        Slot::new(1),
        Ok(profile::Info {
            name: String::try_from("Typing").unwrap(),
            color: white(),
        }),
    );
    round_trip!(GetProfileRevision, Slot::new(1), Ok(u32::MAX));
    round_trip!(Save, (), Err(Rejection::StorageInaccessible));
    round_trip!(Revert, (), DONE);
    round_trip!(FactoryReset, (), DONE);
    round_trip!(EnterUpdateMode, (), ());
    round_trip!(Restart, (), ());
}

#[test]
fn profile_reads_survive_a_round_trip() {
    use openwoot_protocol::endpoints::{
        GetGamepadMapping, GetGamepadSettings, GetKeyOverrides, GetKeySettings,
    };

    round_trip!(GetKeySettings, Slot::new(0), Ok(key_settings()));
    round_trip!(
        GetKeyOverrides,
        Slot::new(1),
        Ok::<_, Rejection>(filled(widest_override(), key_count())),
    );
    round_trip!(
        GetGamepadMapping,
        Slot::new(2),
        Ok(gamepad::Mapping {
            bindings: filled(Some(Control::LeftStickUp), key_count()),
            exclusive: false,
        }),
    );
    round_trip!(GetGamepadSettings, Slot::new(3), Ok(gamepad_settings()));
}

#[test]
fn every_lighting_effect_survives_a_round_trip() {
    use openwoot_protocol::{
        endpoints::GetLighting,
        lighting::{
            AnalogLevel, AnalogRgbFill, Breathing, Effect, HueCycle, HueRange, KeyRipple, KeyTest,
            LevelMeter, PolarGradient, RainbowWave, ReactiveColoring, ReactiveFade, Scanner,
            ScannerDirection, SimplexNoise,
        },
    };

    let level = AnalogLevel {
        base_level: 30,
        gamepad_curve: true,
    };
    for effect in [
        Effect::RainbowWave(RainbowWave {
            hues: HueRange::From214To254,
            period: 9,
            forward: false,
        }),
        Effect::Scanner(Scanner {
            width: 4,
            steps_per_second: 60,
            outside_level: 1,
            direction: ScannerDirection::Down,
        }),
        Effect::Breathing(Breathing {
            step_ms: 20,
            min_level: 5,
        }),
        Effect::HueCycle(HueCycle {
            hues: HueRange::From113To204,
            period: 255,
        }),
        Effect::KeyRipple(KeyRipple {
            palette: true,
            inverted: true,
            steps_per_second: 30,
            decay_step_ms: 6,
            base_level: 2,
        }),
        Effect::ReactiveFade(ReactiveFade {
            base_level: 0,
            step_ms: 8,
            coloring: ReactiveColoring::Palette,
            inverted: true,
        }),
        Effect::LevelMeter(LevelMeter {
            base_level: 7,
            gamepad_curve: false,
        }),
        Effect::AnalogSpotlight(level),
        Effect::AnalogGlow(level),
        Effect::AnalogRgbFill(AnalogRgbFill {
            gamepad_curve: true,
        }),
        Effect::KeyTest(KeyTest { base_level: 64 }),
        Effect::SimplexNoise(SimplexNoise {
            from: opaque(1, 2, 3),
            to: opaque(250, 251, 252),
            speed: 3,
            scale: 200,
        }),
        Effect::PolarGradient(PolarGradient {
            from: opaque(255, 0, 0),
            to: opaque(0, 0, 255),
            speed: -128,
            twist: 127,
        }),
    ] {
        let mut settings = lighting_settings();
        settings.effects = [effect].into_iter().collect();
        round_trip!(GetLighting, Slot::new(0), Ok(settings));
    }
}

#[test]
fn keyboard_wide_reads_runtime_switches_and_rejections_survive_a_round_trip() {
    use openwoot_protocol::{
        configuration::Invalid,
        endpoints::{
            GetDeviceSettings, GetGlobalLightbar, GetGlobalSettings, GetHostSignals, GetLayout,
            GetSwitchModels, LeaveHostControl, ResetHostColors, ResetHostKeyColor,
            SetGamepadDetection, SetKeyReports, SetRawScanning,
        },
    };

    round_trip!(
        GetDeviceSettings,
        (),
        Ok(device::Settings {
            mode: device::Mode::Gamepad,
            keyboard: device::KeyboardFormat::NKeyRollover,
            speed: device::Speed::High,
            gamepad_polling: device::PollingRate::Every4Milliseconds,
        }),
    );
    round_trip!(GetLayout, (), Ok(Layout::Iso));
    round_trip!(
        SetLayout,
        Layout::Jis,
        Err(Rejection::Invalid(Invalid::LayoutMissing(Layout::Jis))),
    );
    round_trip!(
        GetGlobalSettings,
        (),
        Ok(global::Settings {
            idle: None,
            num_lock_inverted: false,
            release_rebound_filter: true,
        }),
    );
    round_trip!(GetGlobalLightbar, (), Ok(lightbar_settings().entries));
    round_trip!(
        GetHostSignals,
        (),
        Ok::<_, Rejection>(filled(u32::MAX, lightbar::MAX_HOST_SIGNALS)),
    );
    round_trip!(
        GetSwitchModels,
        (),
        Ok::<_, Rejection>(filled(
            switches::Model {
                travel: u8::MAX,
                group: u8::MAX,
            },
            switches::MAX_MODELS,
        )),
    );
    round_trip!(
        SetSwitches,
        switches(),
        Err(Rejection::Invalid(Invalid::TravelOffsetReachesTravel {
            model: u8::MAX,
            offset: u8::MAX,
            travel: u8::MAX,
        })),
    );
    round_trip!(
        SetSwitches,
        switches(),
        Err(Rejection::Invalid(Invalid::TunedSwitchesExceedLimit)),
    );
    round_trip!(SetKeyReports, false, DONE);
    round_trip!(SetGamepadDetection, true, DONE);
    round_trip!(SetRawScanning, true, Err(Rejection::Unsupported));
    round_trip!(ResetHostKeyColor, Position { row: 5, column: 16 }, DONE);
    round_trip!(ResetHostColors, (), DONE);
    round_trip!(LeaveHostControl, (), Err(Rejection::Locked));
}
