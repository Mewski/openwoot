#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StackChange {
    Grows(u32),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Instruction {
    Narrow(u16),
    Wide([u16; 2]),
}

impl Instruction {
    pub(super) fn read(bytes: &[u8]) -> Option<Self> {
        let first = bytes.first_chunk().map(|half| u16::from_le_bytes(*half))?;
        if is_wide(first) {
            halves(bytes).map(Self::Wide)
        } else {
            Some(Self::Narrow(first))
        }
    }

    pub(super) const fn width(self) -> usize {
        match self {
            Self::Narrow(_) => 2,
            Self::Wide(_) => 4,
        }
    }

    pub(super) fn stack_change(self) -> Option<StackChange> {
        match self {
            Self::Narrow(instruction) => narrow_stack_change(instruction),
            Self::Wide(instruction) => wide_stack_change(instruction),
        }
    }

    pub(super) const fn writes_basepri(self) -> bool {
        matches!(self, Self::Wide([first, 0x8811 | 0x8812]) if first & 0xFFF0 == 0xF380)
    }

    pub(super) const fn is_branch_with_link(self) -> bool {
        matches!(
            self,
            Self::Wide([first, second]) if first & 0xF800 == 0xF000 && second & 0xD000 == 0xD000
        )
    }

    pub(super) fn unconditional_branch_target(self, at: u32) -> Option<u32> {
        match self {
            Self::Narrow(instruction) if instruction >> 11 == 0b11100 => Some(
                at.wrapping_add(4)
                    .wrapping_add_signed(sign_extend(u32::from(instruction & 0x7FF) << 1, 12)),
            ),
            Self::Wide([first, second])
                if first & 0xF800 == 0xF000 && second & 0xD000 == 0x9000 =>
            {
                Some(branch_target(at, [first, second]))
            }
            _ => None,
        }
    }
}

pub(super) fn halves(bytes: &[u8]) -> Option<[u16; 2]> {
    let [first0, first1, second0, second1] = *bytes.first_chunk()?;
    Some([
        u16::from_le_bytes([first0, first1]),
        u16::from_le_bytes([second0, second1]),
    ])
}

pub(super) const fn is_wide(first: u16) -> bool {
    matches!(first >> 11, 0b11101..=0b11111)
}

pub(super) fn branch_target(at: u32, [first, second]: [u16; 2]) -> u32 {
    let sign = u32::from(first >> 10 & 1);
    let high = u32::from(first & 0x3FF);
    let jump1 = u32::from(second >> 13 & 1);
    let jump2 = u32::from(second >> 11 & 1);
    let low = u32::from(second & 0x7FF);
    let inverted1 = !(jump1 ^ sign) & 1;
    let inverted2 = !(jump2 ^ sign) & 1;
    let offset = sign << 24 | inverted1 << 23 | inverted2 << 22 | high << 12 | low << 1;
    at.wrapping_add(4)
        .wrapping_add_signed(sign_extend(offset, 25))
}

pub(super) fn conditional_branch_target(at: u32, [first, second]: [u16; 2]) -> u32 {
    let sign = u32::from(first >> 10 & 1);
    let high = u32::from(first & 0x3F);
    let jump1 = u32::from(second >> 13 & 1);
    let jump2 = u32::from(second >> 11 & 1);
    let low = u32::from(second & 0x7FF);
    let offset = sign << 20 | jump2 << 19 | jump1 << 18 | high << 12 | low << 1;
    at.wrapping_add(4)
        .wrapping_add_signed(sign_extend(offset, 21))
}

pub(super) fn move_immediate([first, second]: [u16; 2]) -> (u8, u16) {
    let register = ((second >> 8) & 0xF).to_le_bytes()[0];
    let value =
        (first & 0xF) << 12 | (first >> 10 & 1) << 11 | (second >> 12 & 7) << 8 | second & 0xFF;
    (register, value)
}

pub(super) fn narrow_stack_change(instruction: u16) -> Option<StackChange> {
    if instruction & 0xFE00 == 0xB400 {
        let registers = (instruction & 0xFF).count_ones() + u32::from(instruction >> 8 & 1);
        return Some(StackChange::Grows(4 * registers));
    }
    if instruction & 0xFF80 == 0xB080 {
        return Some(StackChange::Grows(4 * u32::from(instruction & 0x7F)));
    }
    if instruction & 0xFF87 == 0x4685 || instruction & 0xFF87 == 0x4485 {
        return Some(StackChange::Unknown);
    }
    None
}

pub(super) fn wide_stack_change([first, second]: [u16; 2]) -> Option<StackChange> {
    let immediate = u32::from(first >> 10 & 1) << 11
        | u32::from(second >> 12 & 7) << 8
        | u32::from(second & 0xFF);
    let grows = match first {
        0xE92D => 4 * u32::from(second & 0x5FFF).count_ones(),
        0xE96D => 4 * u32::from(second & 0xFF),
        0xF84D if second & 0x0F00 == 0x0D00 => u32::from(second & 0xFF),
        _ if first & 0xFFBF == 0xED2D && second & 0x0E00 == 0x0A00 => 4 * u32::from(second & 0xFF),
        _ if first & 0xFBEF == 0xF1AD && second & 0x8F00 == 0x0D00 => expand_immediate(immediate),
        _ if first & 0xFBFF == 0xF2AD && second & 0x8F00 == 0x0D00 => immediate,
        _ if rewrites_stack_pointer(first, second) => return Some(StackChange::Unknown),
        _ => return None,
    };
    Some(StackChange::Grows(grows))
}

const fn rewrites_stack_pointer(first: u16, second: u16) -> bool {
    let by_register = first & 0xFE00 == 0xEA00;
    let by_immediate = first & 0xF800 == 0xF000;
    let releases = first & 0xFBEF == 0xF10D || first & 0xFBFF == 0xF20D;
    second & 0x8F00 == 0x0D00 && (by_register || (by_immediate && !releases))
}

fn expand_immediate(immediate: u32) -> u32 {
    let byte = immediate & 0xFF;
    match immediate >> 8 {
        0 => byte,
        1 => byte << 16 | byte,
        2 => byte << 24 | byte << 8,
        3 => byte << 24 | byte << 16 | byte << 8 | byte,
        _ => (0x80 | (immediate & 0x7F)).rotate_right(immediate >> 7),
    }
}

const fn sign_extend(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    (value << shift).cast_signed() >> shift
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_with_link_reaches_its_target() {
        assert_eq!(branch_target(0x0800_8206, [0xF047, 0xFC6D]), 0x0804_FAE4);
    }

    #[test]
    fn wide_branch_reaches_its_target() {
        assert_eq!(branch_target(0x0800_8832, [0xF002, 0xB974]), 0x0800_AB1E);
    }

    #[test]
    fn conditional_wide_branch_reaches_its_target() {
        assert_eq!(
            conditional_branch_target(0x0800_861C, [0xF040, 0x8666]),
            0x0800_92EC
        );
    }

    #[test]
    fn move_halves_carry_register_and_value() {
        assert_eq!(move_immediate([0xF241, 0x11EE]), (1, 0x11EE));
        assert_eq!(move_immediate([0xF2C8, 0x0100]), (1, 0x8000));
    }

    #[test]
    fn pushes_and_subtractions_grow_the_stack() {
        assert_eq!(narrow_stack_change(0xB5B0), Some(StackChange::Grows(16)));
        assert_eq!(narrow_stack_change(0xB0A0), Some(StackChange::Grows(0x80)));
        assert_eq!(
            wide_stack_change([0xE92D, 0x0B00]),
            Some(StackChange::Grows(12))
        );
        assert_eq!(
            wide_stack_change([0xED2D, 0x8B10]),
            Some(StackChange::Grows(64))
        );
        assert_eq!(
            wide_stack_change([0xF5AD, 0x4D61]),
            Some(StackChange::Grows(0xE100))
        );
        assert_eq!(
            wide_stack_change([0xF6AD, 0x7D14]),
            Some(StackChange::Grows(0xF14))
        );
        assert_eq!(
            wide_stack_change([0xF84D, 0xBD04]),
            Some(StackChange::Grows(4))
        );
    }

    #[test]
    fn subtracting_a_register_from_the_stack_pointer_is_unknown() {
        assert_eq!(
            wide_stack_change([0xEBAD, 0x0D04]),
            Some(StackChange::Unknown)
        );
    }

    #[test]
    fn writing_the_stack_pointer_from_another_register_is_unknown() {
        assert_eq!(narrow_stack_change(0x46BD), Some(StackChange::Unknown));
        assert_eq!(narrow_stack_change(0x4485), Some(StackChange::Unknown));
        assert_eq!(
            wide_stack_change([0xEB0D, 0x0D04]),
            Some(StackChange::Unknown)
        );
        assert_eq!(
            wide_stack_change([0xEA4F, 0x0D07]),
            Some(StackChange::Unknown)
        );
        assert_eq!(
            wide_stack_change([0xF1A7, 0x0D08]),
            Some(StackChange::Unknown)
        );
    }

    #[test]
    fn other_instructions_leave_the_stack_alone() {
        assert_eq!(narrow_stack_change(0xBDB0), None);
        assert_eq!(narrow_stack_change(0xB004), None);
        assert_eq!(narrow_stack_change(0x4639), None);
        assert_eq!(wide_stack_change([0xF10D, 0x0D10]), None);
        assert_eq!(wide_stack_change([0xF20D, 0x4D00]), None);
        assert_eq!(wide_stack_change([0xF241, 0x11EE]), None);
        assert!(is_wide(0xF047) && !is_wide(0xB5B0));
    }

    #[test]
    fn instructions_are_read_with_their_width() {
        assert_eq!(
            Instruction::read(&[0xB0, 0xB5]),
            Some(Instruction::Narrow(0xB5B0))
        );
        assert_eq!(
            Instruction::read(&[0x47, 0xF0, 0x6D, 0xFC]),
            Some(Instruction::Wide([0xF047, 0xFC6D]))
        );
        assert_eq!(Instruction::read(&[0x47, 0xF0]), None);
        assert_eq!(Instruction::read(&[]), None);
        assert_eq!(Instruction::Narrow(0xB5B0).width(), 2);
        assert_eq!(Instruction::Wide([0xF047, 0xFC6D]).width(), 4);
    }

    #[test]
    fn an_instruction_reports_its_stack_change() {
        assert_eq!(
            Instruction::Narrow(0xB5B0).stack_change(),
            Some(StackChange::Grows(16))
        );
        assert_eq!(
            Instruction::Wide([0xE92D, 0x0B00]).stack_change(),
            Some(StackChange::Grows(12))
        );
        assert_eq!(Instruction::Narrow(0xBDB0).stack_change(), None);
    }

    #[test]
    fn only_writes_of_basepri_count_as_masking() {
        assert!(Instruction::Wide([0xF382, 0x8812]).writes_basepri());
        assert!(Instruction::Wide([0xF380, 0x8811]).writes_basepri());
        assert!(!Instruction::Wide([0xF3EF, 0x8011]).writes_basepri());
        assert!(!Instruction::Wide([0xF380, 0x8808]).writes_basepri());
        assert!(!Instruction::Narrow(0xB672).writes_basepri());
    }

    #[test]
    fn only_branches_with_link_count_as_calls() {
        assert!(Instruction::Wide([0xF047, 0xFC6D]).is_branch_with_link());
        assert!(!Instruction::Wide([0xF002, 0xB974]).is_branch_with_link());
        assert!(!Instruction::Wide([0xF040, 0x8666]).is_branch_with_link());
        assert!(!Instruction::Wide([0xF241, 0x11EE]).is_branch_with_link());
        assert!(!Instruction::Narrow(0x4798).is_branch_with_link());
    }

    #[test]
    fn only_unconditional_branches_have_a_jump_target() {
        assert_eq!(
            Instruction::Wide([0xF7FF, 0xBA3E]).unconditional_branch_target(0x0806_F07A),
            Some(0x0806_E4FA)
        );
        assert_eq!(
            Instruction::Narrow(0xE7FE).unconditional_branch_target(0x0800_0100),
            Some(0x0800_0100)
        );
        assert_eq!(
            Instruction::Narrow(0xE002).unconditional_branch_target(0x0800_0100),
            Some(0x0800_0108)
        );
        assert_eq!(
            Instruction::Wide([0xF047, 0xFC6D]).unconditional_branch_target(0x0800_8206),
            None
        );
        assert_eq!(
            Instruction::Wide([0xF040, 0x8666]).unconditional_branch_target(0x0800_861C),
            None
        );
        assert_eq!(
            Instruction::Narrow(0xD002).unconditional_branch_target(0),
            None
        );
    }
}
