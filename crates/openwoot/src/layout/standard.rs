use crate::keys::{Call, Effects, Functions, Notice, Special};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Standard;

impl Functions for Standard {
    fn call<const ROWS: usize, const COLUMNS: usize>(
        &mut self,
        call: Call,
        effects: &mut Effects<'_, ROWS, COLUMNS>,
    ) {
        let Call {
            special,
            key,
            layer,
            pressed,
        } = call;
        match special {
            Special::BrightnessUp | Special::BrightnessDown => {
                effects.notify(Notice::Brightness {
                    increase: special == Special::BrightnessUp,
                    held: pressed,
                });
            }
            Special::Layer(layer) => {
                let active = effects.layers().momentary(layer, pressed);
                effects.notify(Notice::ShowLayer(active));
            }
            _ if !pressed => {}
            Special::Profile(profile) => {
                if effects.profiles().select(profile) {
                    effects.ignore_until_release(key);
                    effects.notify(Notice::ProfileBlink(key));
                }
            }
            Special::ToggleModeProfile => {
                effects.ignore_until_release(key);
                effects.profiles().toggle_mode();
                effects.notify(Notice::StopProfileBlink);
            }
            Special::NextProfile => {
                effects.ignore_until_release(key);
                effects.profiles().next();
            }
            Special::ToggleKeyDisable => {
                if let Some(disabled) = effects.toggle_disabled(key) {
                    effects.notify(Notice::Disabled { key, disabled });
                }
            }
            Special::LayerLock => {
                if let Some(locked) = effects.layers().lock(layer) {
                    effects.notify(Notice::ShowLayer(locked));
                }
            }
        }
    }
}
