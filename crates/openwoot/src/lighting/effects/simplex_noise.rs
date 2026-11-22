use openwoot_protocol::lighting::SimplexNoise;

use crate::lighting::{
    canvas::Canvas,
    color::{blend, opaque},
};

const SKEW: u64 = 1_431_655_764;
const UNSKEW: u64 = 715_827_884;
const ONE: i32 = 1 << 14;
const RADIUS_SQUARED: i32 = 161_061_274;
const OUTPUT_SCALE: u32 = 16_748;
const FIRST_OFFSET: i32 = 2730;
const SECOND_OFFSET: i32 = 5461;
const THIRD_OFFSET: i32 = 8192;

const PERMUTATION: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69,
    142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219,
    203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60, 211, 133, 230,
    220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1, 216, 80, 73, 209, 76,
    132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173,
    186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206,
    59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163,
    70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232,
    178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162,
    241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157, 184, 84, 204,
    176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141,
    128, 195, 78, 66, 215, 61, 156, 180,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State {
    config: SimplexNoise,
    time: u32,
}

impl State {
    pub(super) const fn new(config: SimplexNoise) -> Self {
        Self { config, time: 0 }
    }

    pub(super) const fn update(&mut self, config: SimplexNoise) {
        *self = Self::new(config);
    }

    pub(super) fn render<const ROWS: usize, const COLUMNS: usize>(
        &mut self,
        canvas: &mut impl Canvas,
        elapsed_ms: u32,
    ) {
        let advance = elapsed_ms
            .wrapping_mul(u32::from(self.config.speed))
            .cast_signed()
            >> 5;
        self.time = self.time.wrapping_add(advance.cast_unsigned());
        let key_step = u32::from(self.config.scale) << 4;
        let from = opaque(self.config.from);
        let to = opaque(self.config.to);
        let mut x = 0_u32;
        for row in 0..ROWS {
            let mut y = 0_u32;
            for column in 0..COLUMNS {
                canvas.set_key(row, column, blend(from, to, noise(x, y, self.time)));
                y = y.wrapping_add(key_step);
            }
            x = x.wrapping_add(key_step);
        }
    }
}

pub(super) fn noise(px: u32, py: u32, pz: u32) -> u8 {
    let skew = ((u64::from(px) + u64::from(py) + u64::from(pz)) * SKEW) >> 32;
    let half_skew = low_32(skew) >> 1;
    let cell = |coordinate: u32| ((coordinate >> 1).wrapping_add(half_skew)) >> 11;
    let (ci, cj, ck) = (cell(px), cell(py), cell(pz));
    let unskew = (u64::from(ci) + u64::from(cj) + u64::from(ck)) * UNSKEW;
    let distance = |coordinate: u32, origin: u32| {
        let origin = (u64::from(origin) << 32).wrapping_sub(unskew) >> 18;
        (coordinate << 2).wrapping_sub(low_32(origin)).cast_signed()
    };
    let (x0, y0, z0) = (distance(px, ci), distance(py, cj), distance(pz, ck));

    let (first, second) = if x0 >= y0 {
        if y0 >= z0 {
            ((1, 0, 0), (1, 1, 0))
        } else if x0 >= z0 {
            ((1, 0, 0), (1, 0, 1))
        } else {
            ((0, 0, 1), (1, 0, 1))
        }
    } else if y0 < z0 {
        ((0, 0, 1), (0, 1, 1))
    } else if x0 < z0 {
        ((0, 1, 0), (0, 1, 1))
    } else {
        ((0, 1, 0), (1, 1, 0))
    };
    let corner_hash = |(di, dj, dk): (u32, u32, u32)| {
        permute(
            ci.wrapping_add(di).wrapping_add(u32::from(permute(
                cj.wrapping_add(dj)
                    .wrapping_add(u32::from(permute(ck.wrapping_add(dk)))),
            ))),
        )
    };
    let offset = |base: i32, step: u32, shift: i32| {
        base.wrapping_sub(step.cast_signed() << 14)
            .wrapping_add(shift)
    };

    let mut sum = corner(corner_hash((0, 0, 0)), x0, y0, z0);
    sum = sum.wrapping_add(corner(
        corner_hash(first),
        offset(x0, first.0, FIRST_OFFSET),
        offset(y0, first.1, FIRST_OFFSET),
        offset(z0, first.2, FIRST_OFFSET),
    ));
    sum = sum.wrapping_add(corner(
        corner_hash(second),
        offset(x0, second.0, SECOND_OFFSET),
        offset(y0, second.1, SECOND_OFFSET),
        offset(z0, second.2, SECOND_OFFSET),
    ));
    sum = sum.wrapping_add(corner(
        corner_hash((1, 1, 1)),
        x0.wrapping_sub(ONE).wrapping_add(THIRD_OFFSET),
        y0.wrapping_sub(ONE).wrapping_add(THIRD_OFFSET),
        z0.wrapping_sub(ONE).wrapping_add(THIRD_OFFSET),
    ));
    let scaled = (sum >> 8)
        .cast_unsigned()
        .wrapping_mul(OUTPUT_SCALE)
        .wrapping_add(0x8000_0000);
    scaled.to_be_bytes()[0]
}

fn permute(index: u32) -> u8 {
    PERMUTATION
        .get(usize::from(index.to_le_bytes()[0]))
        .copied()
        .unwrap_or_default()
}

fn gradient(hash: u8, dx: i32, dy: i32, dz: i32) -> i32 {
    let selector = hash & 15;
    let mut first = if selector < 8 { dx } else { dy };
    let mut second = match selector {
        0..4 => dy,
        12 | 14 => dx,
        _ => dz,
    };
    if selector & 1 != 0 {
        first = first.wrapping_neg();
    }
    if selector & 2 != 0 {
        second = second.wrapping_neg();
    }
    first.wrapping_add(second)
}

fn corner(hash: u8, dx: i32, dy: i32, dz: i32) -> i32 {
    let falloff = RADIUS_SQUARED
        .wrapping_sub(dx.wrapping_mul(dx))
        .wrapping_sub(dy.wrapping_mul(dy))
        .wrapping_sub(dz.wrapping_mul(dz))
        >> 12;
    if falloff <= 0 {
        return 0;
    }
    let falloff = falloff.wrapping_mul(falloff) >> 16;
    let falloff = falloff.wrapping_mul(falloff) >> 16;
    falloff.wrapping_mul(gradient(hash, dx, dy, dz))
}

const fn low_32(value: u64) -> u32 {
    let [b0, b1, b2, b3, ..] = value.to_le_bytes();
    u32::from_le_bytes([b0, b1, b2, b3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_corner_offsets_are_multiples_of_the_unskew_factor() {
        assert_eq!(
            i64::from(FIRST_OFFSET),
            i64::try_from(UNSKEW >> 18).unwrap()
        );
        assert_eq!(
            i64::from(SECOND_OFFSET),
            i64::try_from((2 * UNSKEW) >> 18).unwrap()
        );
        assert_eq!(
            i64::from(THIRD_OFFSET),
            i64::try_from((3 * UNSKEW) >> 18).unwrap()
        );
    }

    #[test]
    fn noise_is_centred_on_128_at_the_origin() {
        assert_eq!(noise(0, 0, 0), 128);
    }
}
