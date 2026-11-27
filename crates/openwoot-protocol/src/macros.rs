use heapless::{String, Vec};
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    configuration::{Capabilities, Invalid, Validate},
    keys::Action,
    wire::{self, Decode, Encode, Writer},
};

pub const MAX_MACROS: u8 = 32;
pub const MAX_STEPS: usize = 256;
pub const NAME_LEN: usize = 32;
pub const TEXT_LEN: usize = 64;
pub const MAX_ENCODED_LEN: usize = 2000;

pub type Name = String<NAME_LEN>;
pub type Text = String<TEXT_LEN>;
pub type Steps = Vec<Step, MAX_STEPS>;
pub type Summaries = Vec<Option<Summary>, { MAX_MACROS as usize }>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct Macro {
    pub name: Name,
    pub mode: Mode,
    pub steps: Steps,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode,
)]
pub enum Mode {
    #[default]
    Once,
    WhileHeld,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub enum Step {
    Press(Action),
    Release(Action),
    Tap(Action),
    Delay(u16),
    Text(Text),
    MouseMove { x: i16, y: i16 },
    Scroll { vertical: i8, horizontal: i8 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct Summary {
    pub name: Name,
    pub mode: Mode,
    pub steps: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct Change {
    pub index: u8,
    pub value: Macro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoded<'a> {
    bytes: &'a [u8],
    mode: Mode,
    steps: u16,
    first_step: usize,
}

impl Macro {
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        wire::encoded_len(self)
    }
}

impl From<&Macro> for Summary {
    fn from(value: &Macro) -> Self {
        Self {
            name: value.name.clone(),
            mode: value.mode,
            steps: u16::try_from(value.steps.len()).unwrap_or(u16::MAX),
        }
    }
}

impl<'a> Encoded<'a> {
    #[must_use]
    pub fn take(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        let (_, rest) = wire::take::<Name>(bytes)?;
        let (mode, rest) = wire::take::<Mode>(rest)?;
        let (count, mut rest) = wire::take::<usize>(rest)?;
        let steps = u16::try_from(count).ok().filter(|_| count <= MAX_STEPS)?;
        let first_step = bytes.len() - rest.len();
        for _ in 0..count {
            rest = wire::take::<Step>(rest)?.1;
        }
        let bytes = bytes.get(..bytes.len() - rest.len())?;
        Some((
            Self {
                bytes,
                mode,
                steps,
                first_step,
            },
            rest,
        ))
    }

    #[must_use]
    pub fn parse(bytes: &'a [u8]) -> Option<Self> {
        Self::take(bytes).map(|(encoded, _)| encoded)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    #[must_use]
    pub const fn step_count(&self) -> u16 {
        self.steps
    }

    #[must_use]
    pub fn encoded_steps(&self) -> &'a [u8] {
        self.bytes.get(self.first_step..).unwrap_or_default()
    }

    pub fn steps(&self) -> impl Iterator<Item = Step> + use<'a> {
        let mut rest = self.encoded_steps();
        (0..self.steps).map_while(move |_| {
            let (step, after) = wire::take::<Step>(rest)?;
            rest = after;
            Some(step)
        })
    }

    #[must_use]
    pub fn name(&self) -> Name {
        wire::take::<Name>(self.bytes).map_or_else(Name::new, |(name, _)| name)
    }

    #[must_use]
    pub fn summary(&self) -> Summary {
        Summary {
            name: self.name(),
            mode: self.mode,
            steps: self.steps,
        }
    }
}

impl Encode for Encoded<'_> {
    fn encode(&self, writer: &mut Writer<'_>) {
        writer.bytes(self.bytes);
    }
}

impl Validate for Macro {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        check_len(self.encoded_len())?;
        self.steps
            .iter()
            .try_for_each(|step| step.validate(capabilities))
    }
}

impl Validate for Encoded<'_> {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        check_len(self.bytes.len())?;
        self.steps()
            .try_for_each(|step| step.validate(capabilities))
    }
}

impl Validate for Step {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        match self {
            Self::Press(action) | Self::Release(action) | Self::Tap(action) => {
                check_playable(*action, capabilities)
            }
            Self::Text(text) => text
                .chars()
                .find(|character| !is_typable(*character))
                .map_or(Ok(()), |character| {
                    Err(Invalid::CharacterNotTypable(u32::from(character)))
                }),
            Self::Delay(_) | Self::MouseMove { .. } | Self::Scroll { .. } => Ok(()),
        }
    }
}

impl Validate for Change {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        check_index(self.index)?;
        self.value.validate(capabilities)
    }
}

pub fn check_index(index: u8) -> Result<(), Invalid> {
    if index < MAX_MACROS {
        Ok(())
    } else {
        Err(Invalid::MacroOutsideRange(index))
    }
}

#[must_use]
pub const fn is_typable(character: char) -> bool {
    matches!(character, ' '..='~' | '\n' | '\t')
}

fn check_playable(action: Action, capabilities: &Capabilities) -> Result<(), Invalid> {
    match action {
        Action::Macro(_) => Err(Invalid::MacroInMacro),
        Action::Gamepad(_) | Action::Layer(_) | Action::Function(_) => {
            Err(Invalid::ActionNotPlayable)
        }
        Action::None
        | Action::Keyboard(_)
        | Action::Consumer(_)
        | Action::System(_)
        | Action::Mouse(_) => action.validate(capabilities),
    }
}

fn check_len(len: usize) -> Result<(), Invalid> {
    if len <= MAX_ENCODED_LEN {
        Ok(())
    } else {
        Err(Invalid::MacroTooLong(
            u16::try_from(len).unwrap_or(u16::MAX),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        device::Layout,
        keys::{Function, Layer, MouseButton, SystemControl},
    };

    type Bytes = Vec<u8, 4096>;

    fn capabilities() -> Capabilities {
        Capabilities {
            key_rows: 6,
            key_columns: 17,
            profiles: 4,
            linked_profiles: 10,
            layouts: [Layout::Ansi].into_iter().collect(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 1,
            edition: None,
        }
    }

    fn text(value: &str) -> Step {
        Step::Text(Text::try_from(value).unwrap())
    }

    fn sample() -> Macro {
        Macro {
            name: Name::try_from("Greeting").unwrap(),
            mode: Mode::WhileHeld,
            steps: [
                Step::Press(Action::Keyboard(0xE1)),
                Step::Tap(Action::Keyboard(0x04)),
                Step::Release(Action::Keyboard(0xE1)),
                Step::Delay(300),
                text("Hi!\n"),
                Step::MouseMove { x: -300, y: 2 },
                Step::Scroll {
                    vertical: -3,
                    horizontal: 1,
                },
                Step::Tap(Action::Consumer(0x00E9)),
                Step::Tap(Action::Mouse(MouseButton::Left)),
            ]
            .into_iter()
            .collect(),
        }
    }

    fn bytes(value: &impl Encode) -> Bytes {
        let mut buffer = [0; 4096];
        let len = wire::encode(value, &mut buffer).unwrap();
        Bytes::from_slice(&buffer[..len]).unwrap()
    }

    fn filled(step: &Step) -> Macro {
        Macro {
            name: Name::new(),
            mode: Mode::Once,
            steps: core::iter::repeat_n(step.clone(), MAX_STEPS).collect(),
        }
    }

    fn too_long(value: &Macro) -> Result<(), Invalid> {
        Err(Invalid::MacroTooLong(
            u16::try_from(value.encoded_len()).unwrap(),
        ))
    }

    #[test]
    fn a_macro_in_its_wire_form_reads_like_the_decoded_macro() {
        let encoded_macro = bytes(&sample());
        let mut followed = encoded_macro.clone();
        followed.extend_from_slice(&[7, 8]).unwrap();
        let (encoded, rest) = Encoded::take(&followed).unwrap();
        assert_eq!(encoded.as_bytes(), encoded_macro.as_slice());
        assert_eq!(rest, [7, 8]);
        assert_eq!(encoded.mode(), Mode::WhileHeld);
        assert_eq!(encoded.step_count(), 9);
        assert!(encoded.steps().eq(sample().steps));
        assert_eq!(encoded.summary(), Summary::from(&sample()));
        assert_eq!(bytes(&encoded), encoded_macro);
        assert_eq!(sample().encoded_len(), encoded_macro.len());
        assert_eq!(
            wire::take::<Step>(encoded.encoded_steps()).map(|(step, _)| step),
            Some(Step::Press(Action::Keyboard(0xE1)))
        );
    }

    #[test]
    fn more_steps_than_a_macro_holds_do_not_decode() {
        let mut encoded = bytes(&filled(&Step::Delay(1)));
        assert_eq!(encoded[2..4], [0x80, 0x02]);
        encoded[2] = 0x81;
        encoded.extend_from_slice(&bytes(&Step::Delay(1))).unwrap();
        assert_eq!(Encoded::take(&encoded), None);
        assert_eq!(wire::take::<Macro>(&encoded), None);
    }

    fn judged_alike(value: &Macro, expected: Result<(), Invalid>) {
        let capabilities = capabilities();
        assert_eq!(value.validate(&capabilities), expected, "{value:?}");
        let encoded = bytes(value);
        let parsed = Encoded::parse(&encoded).unwrap();
        assert_eq!(parsed.validate(&capabilities), expected, "{value:?}");
    }

    #[test]
    fn the_wire_form_and_the_value_judge_a_macro_alike() {
        judged_alike(&sample(), Ok(()));
        judged_alike(
            &filled(&Step::Tap(Action::Macro(3))),
            Err(Invalid::MacroInMacro),
        );
        judged_alike(
            &filled(&Step::Press(Action::Layer(Layer::One))),
            Err(Invalid::ActionNotPlayable),
        );
        judged_alike(
            &filled(&Step::Release(Action::Function(Function::LayerLock))),
            Err(Invalid::ActionNotPlayable),
        );
        judged_alike(
            &filled(&Step::Tap(Action::Keyboard(0x32))),
            Err(Invalid::KeyboardUsageOutsideReport(0x32)),
        );
        let long_text = filled(&text("tab\tand é"));
        judged_alike(&long_text, too_long(&long_text));
        judged_alike(&filled(&Step::Delay(u16::MAX)), Ok(()));
        judged_alike(
            &filled(&Step::MouseMove {
                x: i16::MIN,
                y: i16::MAX,
            }),
            Ok(()),
        );
        judged_alike(
            &Macro {
                steps: [text("née")].into_iter().collect(),
                ..sample()
            },
            Err(Invalid::CharacterNotTypable(0xE9)),
        );
        judged_alike(
            &Macro {
                steps: [Step::Tap(Action::System(SystemControl::Sleep)), text("\r")]
                    .into_iter()
                    .collect(),
                ..sample()
            },
            Err(Invalid::CharacterNotTypable(0x0D)),
        );
    }

    #[test]
    fn a_macro_of_exactly_the_largest_size_is_accepted() {
        let mut name = Name::new();
        while name.push('W').is_ok() {}
        let mut full = Text::new();
        while full.push('~').is_ok() {}
        let mut value = Macro {
            name,
            mode: Mode::Toggle,
            steps: Steps::new(),
        };
        let text_len = wire::encoded_len(&Step::Text(full.clone()));
        while value.encoded_len() + text_len <= MAX_ENCODED_LEN {
            value.steps.push(Step::Text(full.clone())).unwrap();
        }
        while value.encoded_len() + 2 <= MAX_ENCODED_LEN {
            value.steps.push(Step::Delay(0)).unwrap();
        }
        if value.encoded_len() < MAX_ENCODED_LEN
            && let Some(last) = value.steps.last_mut()
        {
            *last = Step::Delay(0x80);
        }
        assert_eq!(value.encoded_len(), MAX_ENCODED_LEN);
        assert_eq!(value.validate(&capabilities()), Ok(()));
        value.steps.push(Step::Delay(0)).unwrap();
        assert_eq!(value.validate(&capabilities()), too_long(&value));
    }

    #[test]
    fn a_macro_slot_lies_below_the_slot_count() {
        assert_eq!(check_index(31), Ok(()));
        assert_eq!(check_index(32), Err(Invalid::MacroOutsideRange(32)));
        let change = Change {
            index: MAX_MACROS - 1,
            value: sample(),
        };
        assert_eq!(change.validate(&capabilities()), Ok(()));
        assert_eq!(
            Change {
                index: MAX_MACROS,
                ..change
            }
            .validate(&capabilities()),
            Err(Invalid::MacroOutsideRange(MAX_MACROS))
        );
        assert_eq!(
            Action::Macro(MAX_MACROS).validate(&capabilities()),
            Err(Invalid::MacroOutsideRange(MAX_MACROS))
        );
        assert_eq!(Action::Macro(0).validate(&capabilities()), Ok(()));
    }

    #[test]
    fn printable_ascii_newline_and_tab_are_typable_and_nothing_else() {
        let typable = (0..=0x7F_u32)
            .filter_map(char::from_u32)
            .filter(|character| is_typable(*character))
            .count();
        assert_eq!(typable, 95 + 2);
        assert!(is_typable('\t') && is_typable('\n') && is_typable(' ') && is_typable('~'));
        assert!(!is_typable('\u{7F}') && !is_typable('\r') && !is_typable('é'));
    }
}
