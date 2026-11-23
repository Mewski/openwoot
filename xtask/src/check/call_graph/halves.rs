use std::collections::{HashMap, hash_map::Entry};

use anyhow::{Context, Result, bail, ensure};
use openwoot_image::Hex;

use super::thumb::Instruction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Half {
    pub(super) at: u32,
    pub(super) register: u8,
    pub(super) value: u16,
    pub(super) symbol: u32,
}

pub(super) fn join(
    code: &[(u32, Instruction)],
    lows: &[Half],
    highs: &[Half],
) -> Result<Vec<(u32, u32)>> {
    let index: HashMap<u32, usize> = code
        .iter()
        .enumerate()
        .map(|(position, (address, _))| (*address, position))
        .collect();
    let low_at: HashMap<u32, &Half> = lows.iter().map(|low| (low.at, low)).collect();
    let high_at: HashMap<u32, &Half> = highs.iter().map(|high| (high.at, high)).collect();
    let mut joined: HashMap<u32, &Half> = HashMap::new();
    for low in lows {
        let high = upper_half(code, &index, &low_at, &high_at, low)?;
        ensure!(
            high.symbol == low.symbol,
            "the halves of an address at {} and {} name different symbols",
            Hex(low.at),
            Hex(high.at)
        );
        match joined.entry(high.at) {
            Entry::Vacant(entry) => {
                entry.insert(low);
            }
            Entry::Occupied(entry) => ensure!(
                entry.get().value == low.value,
                "the upper half of an address at {} completes different lower halves at {} and {}",
                Hex(high.at),
                Hex(entry.get().at),
                Hex(low.at)
            ),
        }
    }
    highs
        .iter()
        .map(|high| {
            let low = joined.get(&high.at).with_context(|| {
                format!(
                    "the upper half of an address at {} has no lower half",
                    Hex(high.at)
                )
            })?;
            Ok((high.at, u32::from(high.value) << 16 | u32::from(low.value)))
        })
        .collect()
}

fn upper_half<'h>(
    code: &[(u32, Instruction)],
    index: &HashMap<u32, usize>,
    low_at: &HashMap<u32, &Half>,
    high_at: &HashMap<u32, &'h Half>,
    low: &Half,
) -> Result<&'h Half> {
    let Some(mut position) = index.get(&low.at).copied() else {
        bail!(
            "the lower half of an address at {} is not an instruction of its function",
            Hex(low.at)
        );
    };
    for _ in 0..code.len() {
        let Some(&(address, instruction)) = code.get(position) else {
            break;
        };
        if address != low.at {
            if let Some(high) = high_at
                .get(&address)
                .filter(|high| high.register == low.register)
            {
                return Ok(high);
            }
            ensure!(
                low_at
                    .get(&address)
                    .is_none_or(|other| other.register != low.register),
                "the lower half of an address at {} is replaced at {} before an upper half",
                Hex(low.at),
                Hex(address)
            );
        }
        position = match instruction.unconditional_branch_target(address) {
            Some(target) => index.get(&target).copied().with_context(|| {
                format!(
                    "the lower half of an address at {} is followed by a branch out of its function at {}",
                    Hex(low.at),
                    Hex(address)
                )
            })?,
            None => position + 1,
        };
    }
    bail!(
        "the lower half of an address at {} has no upper half",
        Hex(low.at)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOVW_R6: Instruction = Instruction::Wide([0xF644, 0x2660]);
    const MOVT_R6: Instruction = Instruction::Wide([0xF2C2, 0x0600]);
    const LDR_R5: Instruction = Instruction::Narrow(0x9D09);
    const STRB: Instruction = Instruction::Wide([0xF886, 0x10CA]);
    const BRANCH_BACK: Instruction = Instruction::Wide([0xF7FF, 0xBA3E]);
    const NOP: Instruction = Instruction::Narrow(0xBF00);

    const fn half(at: u32, value: u16) -> Half {
        Half {
            at,
            register: 6,
            value,
            symbol: 7,
        }
    }

    fn shared_upper_half() -> Vec<(u32, Instruction)> {
        vec![
            (0x0806_E4F6, MOVW_R6),
            (0x0806_E4FA, MOVT_R6),
            (0x0806_E4FE, STRB),
            (0x0806_F074, MOVW_R6),
            (0x0806_F078, LDR_R5),
            (0x0806_F07A, BRANCH_BACK),
            (0x0806_F07E, NOP),
        ]
    }

    #[test]
    fn a_lower_half_is_completed_by_the_next_upper_half_of_its_register() {
        let code = [(0x100, MOVW_R6), (0x104, LDR_R5), (0x106, MOVT_R6)];
        assert_eq!(
            join(&code, &[half(0x100, 0x4A60)], &[half(0x106, 0x2000)]).unwrap(),
            [(0x106, 0x2000_4A60)]
        );
    }

    #[test]
    fn lower_halves_may_branch_to_a_shared_upper_half() {
        assert_eq!(
            join(
                &shared_upper_half(),
                &[half(0x0806_E4F6, 0x4A60), half(0x0806_F074, 0x4A60)],
                &[half(0x0806_E4FA, 0x2000)],
            )
            .unwrap(),
            [(0x0806_E4FA, 0x2000_4A60)]
        );
    }

    #[test]
    fn a_shared_upper_half_must_complete_one_address() {
        let error = join(
            &shared_upper_half(),
            &[half(0x0806_E4F6, 0x4A60), half(0x0806_F074, 0x4A64)],
            &[half(0x0806_E4FA, 0x2000)],
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "the upper half of an address at 0x0806_e4fa completes different lower halves at 0x0806_e4f6 and 0x0806_f074"
        );
        let other_symbol = Half {
            symbol: 8,
            ..half(0x0806_F074, 0x4A60)
        };
        assert!(
            join(
                &shared_upper_half(),
                &[half(0x0806_E4F6, 0x4A60), other_symbol],
                &[half(0x0806_E4FA, 0x2000)],
            )
            .is_err()
        );
    }

    #[test]
    fn a_lower_half_without_an_upper_half_on_its_path_is_rejected() {
        let mut code = shared_upper_half();
        code.truncate(5);
        assert_eq!(
            join(
                &code,
                &[half(0x0806_E4F6, 0x4A60), half(0x0806_F074, 0x4A60)],
                &[half(0x0806_E4FA, 0x2000)],
            )
            .unwrap_err()
            .to_string(),
            "the lower half of an address at 0x0806_f074 has no upper half"
        );
        let other_register = Half {
            register: 5,
            ..half(0x0806_F074, 0x4A60)
        };
        assert!(
            join(
                &shared_upper_half(),
                &[half(0x0806_E4F6, 0x4A60), other_register],
                &[half(0x0806_E4FA, 0x2000)],
            )
            .is_err()
        );
    }

    #[test]
    fn a_branch_out_of_the_function_or_a_loop_is_rejected() {
        let outside = [(0x0806_F074, MOVW_R6), (0x0806_F07A, BRANCH_BACK)];
        assert_eq!(
            join(&outside, &[half(0x0806_F074, 0x4A60)], &[])
                .unwrap_err()
                .to_string(),
            "the lower half of an address at 0x0806_f074 is followed by a branch out of its function at 0x0806_f07a"
        );
        let looping = [(0x100, MOVW_R6), (0x104, Instruction::Narrow(0xE7FE))];
        assert_eq!(
            join(&looping, &[half(0x100, 0x4A60)], &[])
                .unwrap_err()
                .to_string(),
            "the lower half of an address at 0x0000_0100 has no upper half"
        );
    }

    #[test]
    fn a_replaced_lower_half_and_an_unreached_upper_half_are_rejected() {
        let code = [(0x100, MOVW_R6), (0x104, MOVW_R6), (0x108, MOVT_R6)];
        assert_eq!(
            join(
                &code,
                &[half(0x100, 0x4A60), half(0x104, 0x4A60)],
                &[half(0x108, 0x2000)]
            )
            .unwrap_err()
            .to_string(),
            "the lower half of an address at 0x0000_0100 is replaced at 0x0000_0104 before an upper half"
        );
        assert_eq!(
            join(&code[2..], &[], &[half(0x108, 0x2000)])
                .unwrap_err()
                .to_string(),
            "the upper half of an address at 0x0000_0108 has no lower half"
        );
    }
}
