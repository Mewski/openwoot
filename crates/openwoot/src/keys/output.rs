use heapless::Vec;
use openwoot_protocol::keys::{Action, MouseButton, SystemControl};

use super::{GamepadInput, Key, Notice, Special, follows_travel, gamepad_code};
use crate::{
    hid::{
        self, BootReport, ConsumerReport, KeyStateEntry, KeyStateKind, KeyStateReport, MouseReport,
        NkroReport, SystemReport,
    },
    lighting::TRAVEL_FULL,
    xinput,
};

pub(crate) const NOTICE_CAPACITY: usize = 64;

const KEY_STATE_TRAVEL_FULL: u32 = 1023;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamepadReport {
    Xinput(xinput::Report),
    Hid(hid::GamepadReport),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    boot_format: bool,
    boot: BootReport,
    nkro: NkroReport,
    system: SystemReport,
    consumer: ConsumerReport,
    consumer_len: usize,
    mouse: MouseReport,
    key_states: KeyStateReport,
    gamepad_input: GamepadInput,
    gamepad: Option<GamepadReport>,
    notices: Vec<Notice, NOTICE_CAPACITY>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyboard {
    Boot(BootReport),
    NKeyRollover(NkroReport),
}

impl Output {
    #[must_use]
    pub const fn keyboard(&self) -> Keyboard {
        if self.boot_format {
            Keyboard::Boot(self.boot)
        } else {
            Keyboard::NKeyRollover(self.nkro)
        }
    }

    #[must_use]
    pub const fn system(&self) -> &SystemReport {
        &self.system
    }

    #[must_use]
    pub const fn consumer(&self) -> &ConsumerReport {
        &self.consumer
    }

    #[must_use]
    pub fn consumer_usages(&self) -> &[u16] {
        self.consumer
            .usages
            .get(..self.consumer_len)
            .unwrap_or_default()
    }

    #[must_use]
    pub const fn mouse(&self) -> &MouseReport {
        &self.mouse
    }

    #[must_use]
    pub const fn key_states(&self) -> &KeyStateReport {
        &self.key_states
    }

    #[must_use]
    pub const fn gamepad_input(&self) -> &GamepadInput {
        &self.gamepad_input
    }

    #[must_use]
    pub const fn gamepad(&self) -> Option<&GamepadReport> {
        self.gamepad.as_ref()
    }

    #[must_use]
    pub fn notices(&self) -> &[Notice] {
        &self.notices
    }

    pub(super) fn clear(&mut self, boot_format: bool) {
        let notices = core::mem::take(&mut self.notices);
        *self = Self {
            boot_format,
            notices,
            ..Self::default()
        };
    }

    pub(super) fn take_notices(&mut self) -> Vec<Notice, NOTICE_CAPACITY> {
        core::mem::take(&mut self.notices)
    }

    pub(super) fn notify(&mut self, notice: Notice) {
        if self.notices.is_full() && !matches!(notice, Notice::Key { .. }) {
            let mut carried = notice;
            for queued in self.notices.iter_mut().rev() {
                core::mem::swap(queued, &mut carried);
                if matches!(carried, Notice::Key { .. }) {
                    return;
                }
            }
            return;
        }
        let _ = self.notices.push(notice);
    }

    pub(super) const fn gamepad_input_mut(&mut self) -> &mut GamepadInput {
        &mut self.gamepad_input
    }

    pub(super) const fn set_gamepad(&mut self, report: GamepadReport) {
        self.gamepad = Some(report);
    }

    pub(super) fn press_keyboard(&mut self, usage: u8) {
        if self.boot_format {
            self.boot.press(usage);
        } else {
            self.nkro.set(usage, true);
        }
    }

    pub(super) fn press_consumer(&mut self, usage: u16) {
        if let Some(slot) = self.consumer.usages.get_mut(self.consumer_len) {
            *slot = usage;
            self.consumer_len += 1;
        }
    }

    pub(super) const fn press_system(&mut self, control: SystemControl) {
        self.system.control = Some(match control {
            SystemControl::PowerDown => hid::SystemControl::PowerDown,
            SystemControl::Sleep => hid::SystemControl::Sleep,
            SystemControl::WakeUp => hid::SystemControl::WakeUp,
        });
    }

    pub(super) const fn press_mouse(&mut self, button: MouseButton) {
        self.mouse.buttons |= 1 << mouse_bit(button);
    }

    pub(super) fn log(&mut self, action: Action, key: Key, travel: u16, pressed: bool) {
        let (code, kind) = key_state_code(action);
        self.log_entry(key, code, kind, travel, pressed);
    }

    pub(super) fn log_advanced(&mut self, kind: u8, key: Key, travel: u16, pressed: bool) {
        self.log_entry(key, kind, KeyStateKind::Advanced, travel, pressed);
    }

    fn log_entry(&mut self, key: Key, code: u8, kind: KeyStateKind, travel: u16, pressed: bool) {
        let scaled = KEY_STATE_TRAVEL_FULL * u32::from(travel) / u32::from(TRAVEL_FULL);
        let _ = self.key_states.push(KeyStateEntry {
            key: key.code(),
            code,
            pressed,
            kind,
            travel: u16::try_from(scaled)
                .ok()
                .and_then(hid::Travel::new)
                .unwrap_or(hid::Travel::MAX),
        });
    }
}

const fn mouse_bit(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
    }
}

const fn system_code(control: SystemControl) -> u8 {
    match control {
        SystemControl::PowerDown => 1,
        SystemControl::Sleep => 2,
        SystemControl::WakeUp => 3,
    }
}

fn key_state_code(action: Action) -> (u8, KeyStateKind) {
    match action {
        Action::None => (0, KeyStateKind::Function),
        Action::Keyboard(usage) => (usage, KeyStateKind::Keyboard),
        Action::Consumer(usage) => {
            let [low, _] = usage.to_le_bytes();
            (low, KeyStateKind::Consumer)
        }
        Action::System(control) => (system_code(control), KeyStateKind::Consumer),
        Action::Mouse(button) => (mouse_bit(button) + MOUSE_FIRST_CODE, KeyStateKind::Consumer),
        Action::Layer(_) | Action::Function(_) => (
            Special::from_action(action).map_or(0, Special::code),
            KeyStateKind::Function,
        ),
        Action::Gamepad(control) => (gamepad_code(control), KeyStateKind::Gamepad),
    }
}

const MOUSE_FIRST_CODE: u8 = 4;

#[must_use]
pub(crate) fn follows_travel_action(action: Action) -> bool {
    matches!(action, Action::Gamepad(control) if follows_travel(control))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use openwoot_protocol::gamepad::Control;

    use super::*;
    use crate::hid::CONSUMER_SLOTS;

    #[test]
    fn key_state_entries_carry_the_key_codes_and_ten_bit_travel() {
        let mut output = Output::default();
        let key = Key::new(1, 2);
        output.log(Action::Keyboard(0xE1), key, 4095, true);
        output.log(Action::Mouse(MouseButton::Forward), key, 0, false);
        output.log(Action::None, key, 2047, true);
        output.log(Action::Gamepad(Control::HatRight), key, 1, true);
        output.log(Action::Consumer(0x0183), key, 4, true);
        let entries: std::vec::Vec<_> = output
            .key_states()
            .entries()
            .iter()
            .map(|entry| (entry.key, entry.code, entry.kind, entry.travel.get()))
            .collect();
        assert_eq!(
            entries,
            [
                (0x22, 0xE1, KeyStateKind::Keyboard, 1023),
                (0x22, 8, KeyStateKind::Consumer, 0),
                (0x22, 0, KeyStateKind::Function, 511),
                (0x22, 155, KeyStateKind::Gamepad, 0),
                (0x22, 0x83, KeyStateKind::Consumer, 0),
            ]
        );
    }

    #[test]
    fn the_consumer_report_holds_three_usages() {
        let mut output = Output::default();
        for usage in [0xB5, 0xB6, 0xCD, 0xE2] {
            output.press_consumer(usage);
        }
        assert_eq!(output.consumer_usages(), [0xB5, 0xB6, 0xCD]);
        assert_eq!(CONSUMER_SLOTS, 3);
    }

    #[test]
    fn a_full_notice_queue_gives_up_a_key_event_for_a_state_notice() {
        let mut output = Output::default();
        for _ in 0..NOTICE_CAPACITY {
            output.notify(Notice::Key {
                key: Key::new(1, 2),
                event: crate::lighting::KeyEvent::Pressed,
            });
        }
        output.notify(Notice::ShowLayer(1));
        output.notify(Notice::ShowLayer(2));
        assert_eq!(output.notices().len(), NOTICE_CAPACITY);
        assert_eq!(
            output.notices()[NOTICE_CAPACITY - 2..],
            [Notice::ShowLayer(1), Notice::ShowLayer(2)]
        );
    }
}
