use core::cmp::{Ordering, Reverse};

use heapless::Vec;

use super::Switch;

const POINTS: usize = 4096;
const COUNT: usize = 40;
const BLOCK_POINTS: usize = 16;
const BLOCKS: usize = POINTS / BLOCK_POINTS;
const POINTS_PER_BYTE: usize = 4;
const RESIDUAL_BITS: usize = 2;
const RESIDUAL_MASK: u8 = 0b11;
const NO_TABLE: u8 = u8::MAX;

const STEPS: &[u8; COUNT * POINTS] = include_bytes!("../../data/travel_curves.txt");
static COMPACT: [Compact; COUNT] = encode(STEPS);

pub struct CurveTable([u16; POINTS]);

#[derive(Debug)]
pub struct CurveCache<'tables> {
    tables: &'tables mut [CurveTable],
    table_of: [u8; COUNT],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Curve(u8);

pub(super) trait Points {
    fn point(&self, index: u32) -> u16;

    fn interpolate(&self, index_q8: u32) -> u16 {
        let index = (index_q8 >> 8) & 0xFFFF;
        let fraction = index_q8 & 0xFF;
        let low = u32::from(self.point(index));
        if fraction == 0 {
            return low as u16;
        }
        let high = u32::from(self.point(index + 1));
        (low + (high
            .wrapping_sub(low)
            .wrapping_mul(fraction)
            .wrapping_add(128)
            >> 8)) as u16
    }
}

pub(super) struct Compact {
    anchors: [u16; BLOCKS + 1],
    residuals: [u8; POINTS / POINTS_PER_BYTE],
}

impl CurveTable {
    pub const EMPTY: Self = Self([0; POINTS]);

    fn fill(&mut self, compact: &Compact) {
        let (residual_words, _) = compact
            .residuals
            .as_chunks::<{ BLOCK_POINTS / POINTS_PER_BYTE }>();
        let anchors = compact.anchors.iter().zip(compact.anchors.iter().skip(1));
        let (blocks, _) = self.0.as_chunks_mut::<BLOCK_POINTS>();
        for ((points, (&low, &high)), &residuals) in
            blocks.iter_mut().zip(anchors).zip(residual_words)
        {
            let mut bits = u32::from_le_bytes(residuals);
            for (offset, point) in points.iter_mut().enumerate() {
                *point = decode(low, high, offset, bits as u8 & RESIDUAL_MASK);
                bits >>= RESIDUAL_BITS;
            }
        }
    }
}

impl Points for CurveTable {
    fn point(&self, index: u32) -> u16 {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.0.get(index))
            .copied()
            .unwrap_or(0)
    }
}

impl core::fmt::Debug for CurveTable {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_struct("CurveTable").finish_non_exhaustive()
    }
}

impl<'tables> CurveCache<'tables> {
    #[must_use]
    pub fn new(tables: &'tables mut [CurveTable]) -> Self {
        let usable = tables.len().min(COUNT);
        Self {
            tables: tables
                .split_at_mut_checked(usable)
                .map_or(&mut [], |(usable, _)| usable),
            table_of: [NO_TABLE; COUNT],
        }
    }

    pub fn assign<'switch>(&mut self, switches: impl IntoIterator<Item = &'switch Switch>) {
        let chosen = self.choose(users(switches));
        for (number, table) in (0..).zip(&mut self.table_of) {
            if !chosen.contains(&Curve(number)) {
                *table = NO_TABLE;
            }
        }
        for &curve in &chosen {
            self.expand(curve);
        }
    }

    #[must_use]
    pub fn travel(
        &self,
        switch: &Switch,
        value: u16,
        rest_level: u16,
        bottom_level: u16,
    ) -> Option<u16> {
        let table = self.table(switch.curve())?;
        Some(switch.travel_over(table, value, rest_level, bottom_level))
    }

    fn table(&self, curve: Curve) -> Option<&CurveTable> {
        let &table = self.table_of.get(usize::from(curve.0))?;
        self.tables.get(usize::from(table))
    }

    fn choose(&self, mut users: Vec<(Curve, u16), COUNT>) -> Vec<Curve, COUNT> {
        let mut chosen = Vec::new();
        for _ in 0..self.tables.len() {
            let best = users
                .iter_mut()
                .filter(|(_, count)| *count != 0)
                .max_by_key(|(curve, count)| {
                    (*count, self.table(*curve).is_some(), Reverse(curve.0))
                });
            let Some((curve, count)) = best else {
                break;
            };
            *count = 0;
            if chosen.push(*curve).is_err() {
                break;
            }
        }
        chosen
    }

    fn expand(&mut self, curve: Curve) {
        if self.table(curve).is_some() {
            return;
        }
        let free = (0_u8..)
            .take(self.tables.len())
            .find(|table| !self.table_of.contains(table));
        if let Some(free) = free
            && let Some(table) = self.tables.get_mut(usize::from(free))
            && let Some(entry) = self.table_of.get_mut(usize::from(curve.0))
        {
            table.fill(curve.compact());
            *entry = free;
        }
    }
}

impl Default for CurveCache<'_> {
    fn default() -> Self {
        Self::new(&mut [])
    }
}

impl Curve {
    pub(super) const END_MAX: u32 = 0xF_FF00;
    pub(super) const VALUE_MAX: u32 = 0x3FFF;

    #[must_use]
    pub(super) const fn number(number: u8) -> Self {
        if (number as usize) < COUNT {
            Self(number)
        } else {
            Self(0)
        }
    }

    #[must_use]
    pub(super) fn compact(self) -> &'static Compact {
        let [first, ..] = &COMPACT;
        COMPACT.get(usize::from(self.0)).unwrap_or(first)
    }
}

impl Compact {
    const EMPTY: Self = Self {
        anchors: [0; BLOCKS + 1],
        residuals: [0; POINTS / POINTS_PER_BYTE],
    };

    #[must_use]
    pub(super) fn end_of_travel(&self, target: u32) -> u32 {
        let last = POINTS as u32 - 1;
        let mut low = 0_u32;
        let mut high = last;
        loop {
            let middle = low + ((high - low) >> 1);
            let point = u32::from(self.point(middle));
            match point.cmp(&target) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => match middle.checked_sub(1) {
                    Some(below) => high = below,
                    None => return 0,
                },
                Ordering::Equal => return self.plateau_end(middle, target),
            }
            if low > high {
                break;
            }
        }
        if low == 0 {
            0
        } else if low > last {
            Curve::END_MAX
        } else {
            let below = u32::from(self.point(high));
            let step = u32::from(self.point(low)).wrapping_sub(below) & 0xFFFF;
            ((step >> 1) + ((target.wrapping_sub(below)) << 8))
                .checked_div(step)
                .map_or(Curve::END_MAX, |position| {
                    (position + (high << 8)).min(Curve::END_MAX)
                })
        }
    }

    fn plateau_end(&self, hit: u32, target: u32) -> u32 {
        let mut first = hit;
        while first != 0 && u32::from(self.point(first - 1)) == target {
            first -= 1;
        }
        let mut last = hit;
        while last + 1 < POINTS as u32 && u32::from(self.point(last + 1)) == target {
            last += 1;
        }
        ((first + (last - first) * target / Curve::VALUE_MAX) << 8).min(Curve::END_MAX)
    }
}

impl Points for Compact {
    fn point(&self, index: u32) -> u16 {
        let Ok(index) = usize::try_from(index) else {
            return 0;
        };
        let block = index / BLOCK_POINTS;
        let (Some(&low), Some(&high), Some(&residuals)) = (
            self.anchors.get(block),
            self.anchors.get(block + 1),
            self.residuals.get(index / POINTS_PER_BYTE),
        ) else {
            return 0;
        };
        let residual = (residuals >> (index % POINTS_PER_BYTE * RESIDUAL_BITS)) & RESIDUAL_MASK;
        decode(low, high, index % BLOCK_POINTS, residual)
    }
}

fn users<'switch>(switches: impl IntoIterator<Item = &'switch Switch>) -> Vec<(Curve, u16), COUNT> {
    let mut users = Vec::<(Curve, u16), COUNT>::new();
    for switch in switches {
        let curve = switch.curve();
        match users.iter_mut().find(|(used, _)| *used == curve) {
            Some((_, count)) => *count = count.saturating_add(1),
            None => {
                if users.push((curve, 1)).is_err() {
                    break;
                }
            }
        }
    }
    users
}

const fn decode(low: u16, high: u16, offset: usize, residual: u8) -> u16 {
    (predict(low, high, offset) + residual as u32 - 1) as u16
}

const fn predict(low: u16, high: u16, offset: usize) -> u32 {
    let low = low as u32;
    low + (((high as u32 - low) * offset as u32 + 8) >> 4)
}

#[expect(
    clippy::indexing_slicing,
    clippy::large_stack_arrays,
    reason = "evaluated at compile time, where an index outside the table fails the build"
)]
const fn encode(steps: &[u8; COUNT * POINTS]) -> [Compact; COUNT] {
    let mut curves = [Compact::EMPTY; COUNT];
    let mut curve = 0;
    while curve < COUNT {
        let points = decode_line(steps, curve);
        let compact = &mut curves[curve];
        let mut block = 0;
        while block < BLOCKS {
            compact.anchors[block] = points[block * BLOCK_POINTS];
            block += 1;
        }
        compact.anchors[BLOCKS] = 2 * points[POINTS - 1] - points[POINTS - 2];
        let mut index = 0;
        while index < POINTS {
            let block = index / BLOCK_POINTS;
            let prediction = predict(
                compact.anchors[block],
                compact.anchors[block + 1],
                index % BLOCK_POINTS,
            );
            let residual = points[index] as i32 + 1 - prediction as i32;
            assert!(
                residual >= 0 && residual <= RESIDUAL_MASK as i32,
                "a point lies between one below and two above the line between its anchors"
            );
            compact.residuals[index / POINTS_PER_BYTE] |=
                (residual as u8) << (index % POINTS_PER_BYTE * RESIDUAL_BITS);
            index += 1;
        }
        curve += 1;
    }
    curves
}

#[expect(
    clippy::indexing_slicing,
    reason = "evaluated at compile time, where an index outside the table fails the build"
)]
const fn decode_line(steps: &[u8; COUNT * POINTS], curve: usize) -> [u16; POINTS] {
    let line = curve * POINTS;
    assert!(
        steps[line + POINTS - 1] == b'\n',
        "one line of steps per curve"
    );
    let mut points = [0; POINTS];
    let mut point = 1;
    while point < POINTS {
        let step = steps[line + point - 1];
        assert!(
            step >= b'A' && step <= b'Z',
            "a step is a letter from `A` (0) upwards"
        );
        points[point] = points[point - 1] + (step - b'A') as u16;
        point += 1;
    }
    points
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{boxed::Box, vec::Vec};

    use super::*;
    use crate::analog::SwitchModel;

    fn full_curves() -> Vec<[u16; POINTS]> {
        let (lines, _) = STEPS.as_chunks::<POINTS>();
        lines
            .iter()
            .map(|line| {
                let mut points = [0; POINTS];
                for point in 1..POINTS {
                    points[point] = points[point - 1] + u16::from(line[point - 1] - b'A');
                }
                points
            })
            .collect()
    }

    fn switch_on(curve: u8) -> Switch {
        let kind = (0..SwitchModel::COUNT)
            .filter_map(SwitchModel::new)
            .find(|kind| kind.curve() == Curve(curve))
            .unwrap();
        Switch::new(kind, None)
    }

    fn tables<const TABLES: usize>() -> Box<[CurveTable; TABLES]> {
        Box::new([CurveTable::EMPTY; TABLES])
    }

    #[test]
    fn every_curve_rises_from_rest_to_full_travel() {
        for number in 0..COUNT as u8 {
            let curve = Curve::number(number).compact();
            assert_eq!(curve.point(0), 0);
            assert_eq!(u32::from(curve.point(POINTS as u32 - 1)), Curve::VALUE_MAX);
            for index in 1..POINTS as u32 {
                assert!(curve.point(index - 1) <= curve.point(index));
            }
        }
    }

    #[test]
    fn interpolation_rounds_between_neighbouring_points() {
        let curve = Curve::number(0).compact();
        assert_eq!(curve.interpolate(0x100), 17);
        assert_eq!(curve.interpolate(0x180), 26);
        assert_eq!(curve.interpolate(0x140), 21);
        assert_eq!(curve.interpolate(Curve::END_MAX), 0x3FFF);
    }

    #[test]
    fn the_end_of_travel_of_the_full_value_is_the_start_of_the_last_plateau() {
        let curve = Curve::number(0).compact();
        let end = curve.end_of_travel(Curve::VALUE_MAX);
        let first = (0..POINTS as u32)
            .find(|&index| u32::from(curve.point(index)) == Curve::VALUE_MAX)
            .unwrap();
        assert_eq!(end, ((first + (4095 - first)) << 8).min(Curve::END_MAX));
        assert_eq!(curve.end_of_travel(0), 0);
    }

    #[test]
    fn the_compact_curves_hold_every_point_of_the_text_file() {
        for (number, points) in (0..).zip(full_curves()) {
            let curve = Curve::number(number).compact();
            for (index, &point) in (0..).zip(&points) {
                assert_eq!(curve.point(index), point, "curve {number}, point {index}");
            }
            assert_eq!(curve.point(POINTS as u32), 0, "curve {number}");
            assert_eq!(curve.point(u32::MAX), 0, "curve {number}");
        }
    }

    #[test]
    fn a_filled_table_holds_every_point_of_the_text_file() {
        let mut table = CurveTable::EMPTY;
        for (number, points) in (0..).zip(full_curves()) {
            table.fill(Curve::number(number).compact());
            assert_eq!(table.0, points, "curve {number}");
            assert_eq!(table.point(POINTS as u32), 0, "curve {number}");
        }
    }

    #[test]
    fn a_curve_number_beyond_the_table_is_the_first_curve() {
        assert_eq!(Curve::number(COUNT as u8), Curve::number(0));
        assert_eq!(Curve::number(u8::MAX), Curve::number(0));
    }

    #[test]
    fn the_most_used_curves_get_the_tables_and_the_rest_stay_compact() {
        let mut tables = tables::<2>();
        let mut cache = CurveCache::new(&mut *tables);
        let (rare, common, most) = (switch_on(1), switch_on(2), switch_on(3));
        cache.assign([&rare, &common, &most, &most, &common, &most]);
        assert_eq!(cache.table_of[1], NO_TABLE);
        let curves = full_curves();
        assert_eq!(cache.table(most.curve()).unwrap().0, curves[3]);
        assert_eq!(cache.table(common.curve()).unwrap().0, curves[2]);
        assert_eq!(cache.travel(&rare, 700, 300, 1200), None);
        assert_eq!(
            cache.travel(&most, 700, 300, 1200),
            Some(most.travel(700, 300, 1200))
        );
    }

    #[test]
    fn a_curve_that_stays_in_use_keeps_its_table_and_a_released_table_takes_a_new_curve() {
        let mut tables = tables::<2>();
        let mut cache = CurveCache::new(&mut *tables);
        let (first, second, third) = (switch_on(4), switch_on(5), switch_on(6));
        cache.assign([&first, &second]);
        assert_eq!((cache.table_of[4], cache.table_of[5]), (0, 1));
        cache.assign([&third, &second, &third]);
        assert_eq!(cache.table_of[4], NO_TABLE);
        assert_eq!((cache.table_of[5], cache.table_of[6]), (1, 0));
        let curves = full_curves();
        assert_eq!(cache.tables[0].0, curves[6]);
        assert_eq!(cache.tables[1].0, curves[5]);
    }

    #[test]
    fn a_tie_keeps_the_curve_that_already_has_a_table() {
        let mut tables = tables::<1>();
        let mut cache = CurveCache::new(&mut *tables);
        let (low, high) = (switch_on(7), switch_on(8));
        cache.assign([&high]);
        cache.assign([&low, &high]);
        assert_eq!(cache.table_of[8], 0);
        assert_eq!(cache.table_of[7], NO_TABLE);
        assert_eq!(cache.tables[0].0, full_curves()[8]);
    }

    #[test]
    fn a_cache_without_tables_and_one_with_more_tables_than_curves_hold_what_they_can() {
        let mut cache = CurveCache::default();
        let switch = switch_on(9);
        cache.assign([&switch]);
        assert_eq!(cache.travel(&switch, 700, 300, 1200), None);

        let mut tables = (0..COUNT + 2)
            .map(|_| CurveTable::EMPTY)
            .collect::<Vec<_>>();
        let mut cache = CurveCache::new(&mut tables);
        assert_eq!(cache.tables.len(), COUNT);
        let switches = (0..COUNT as u8).map(switch_on).collect::<Vec<_>>();
        cache.assign(&switches);
        for switch in &switches {
            assert_eq!(
                cache.travel(switch, 700, 300, 1200),
                Some(switch.travel(700, 300, 1200))
            );
        }
    }
}
