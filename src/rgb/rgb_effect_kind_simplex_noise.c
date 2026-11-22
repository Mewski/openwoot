/*
 * rgb_effect_kind_simplex_noise.c - effect kind 14: two colours blended by a 3D simplex noise
 * field that drifts along its third axis. Every matrix position is overwritten.
 *
 * The noise is FastLED's 3D snoise16() (simplex.cpp) in fixed point: coordinates are .12
 * (4096 = one lattice cell), the permutation table is k_rgb_simplex_perm.
 */
#include "rgb/rgb_effect_layers_internal.h"
#include "rgb/rgb_frame.h"
#include "tables/rgb_color_tables.h"

#define SIMPLEX_F3            1431655764u /* skew factor 1/3 in .32 (0x55555554) */
#define SIMPLEX_G3            715827884u  /* unskew factor 1/6 in .32 (0x2AAAAAAC) */
#define SIMPLEX_ONE           (1 << 14)   /* 1.0 in the .14 format of the corner offsets */
#define SIMPLEX_RADIUS_SQUARE 161061274   /* 0.6 in .28: squared reach of a corner (0x0999999A) */
#define SIMPLEX_OUTPUT_SCALE  16748u      /* scales the corner sum to the full 16-bit range */

static uint8_t simplex_perm(uint32_t index)
{
    return k_rgb_simplex_perm[index & 0xFF];
}

/* Gradient of a lattice corner: one of 12 edge directions selected by the hash. */
static int32_t simplex_grad(uint8_t hash, int32_t x, int32_t y, int32_t z)
{
    uint8_t h = hash & 15;
    int32_t u = h < 8 ? x : y;
    int32_t v = h < 4 ? y : (h == 12 || h == 14) ? x : z;

    if (h & 1) {
        u = -u;
    }
    if (h & 2) {
        v = -v;
    }
    return u + v;
}

/* Contribution of one simplex corner: (0.6 - d^2)^4 * gradient, 0 outside the reach. */
static int32_t simplex_corner(uint8_t hash, int32_t x, int32_t y, int32_t z)
{
    int32_t falloff = (SIMPLEX_RADIUS_SQUARE - x * x - y * y - z * z) >> 12; /* .16 */

    if (falloff <= 0) {
        return 0;
    }
    falloff = (falloff * falloff) >> 16;
    falloff = (falloff * falloff) >> 16;
    return falloff * simplex_grad(hash, x, y, z);
}

/*
 * High byte of FastLED's snoise16(x, y, z): 0..255, 128 = zero crossing. The top byte of
 * the scaled sum is taken directly, without forming the 16-bit result first.
 */
static uint8_t simplex_noise3_high_byte(uint32_t x, uint32_t y, uint32_t z)
{
    /* Skew the input space to find the simplex cell. */
    uint32_t skew = (uint32_t)((((uint64_t)x + y + z) * SIMPLEX_F3) >> 32);
    uint32_t i    = ((x >> 1) + (skew >> 1)) >> 11;
    uint32_t j    = ((y >> 1) + (skew >> 1)) >> 11;
    uint32_t k    = ((z >> 1) + (skew >> 1)) >> 11;

    /* Unskew the cell origin and take the distances from it (.14). */
    uint64_t unskew = ((uint64_t)i + j + k) * SIMPLEX_G3;
    int32_t  x0     = (int32_t)((x << 2) - (uint32_t)((((uint64_t)i << 32) - unskew) >> 18));
    int32_t  y0     = (int32_t)((y << 2) - (uint32_t)((((uint64_t)j << 32) - unskew) >> 18));
    int32_t  z0     = (int32_t)((z << 2) - (uint32_t)((((uint64_t)k << 32) - unskew) >> 18));

    /* Offsets of the second and third corner: order of the three distances. */
    uint32_t i1, j1, k1;
    uint32_t i2, j2, k2;

    if (x0 >= y0) {
        if (y0 >= z0) { /* X Y Z */
            i1 = 1; j1 = 0; k1 = 0;
            i2 = 1; j2 = 1; k2 = 0;
        } else if (x0 >= z0) { /* X Z Y */
            i1 = 1; j1 = 0; k1 = 0;
            i2 = 1; j2 = 0; k2 = 1;
        } else { /* Z X Y */
            i1 = 0; j1 = 0; k1 = 1;
            i2 = 1; j2 = 0; k2 = 1;
        }
    } else {
        if (y0 < z0) { /* Z Y X */
            i1 = 0; j1 = 0; k1 = 1;
            i2 = 0; j2 = 1; k2 = 1;
        } else if (x0 < z0) { /* Y Z X */
            i1 = 0; j1 = 1; k1 = 0;
            i2 = 0; j2 = 1; k2 = 1;
        } else { /* Y X Z */
            i1 = 0; j1 = 1; k1 = 0;
            i2 = 1; j2 = 1; k2 = 0;
        }
    }

    /* Distances from the other three corners (.14): offsets 1/6 = 2730, 2/6 = 5461, 3/6 - 1 = -0x2000. */
    int32_t x1 = x0 - (int32_t)(i1 << 14) + (int32_t)(SIMPLEX_G3 >> 18);
    int32_t y1 = y0 - (int32_t)(j1 << 14) + (int32_t)(SIMPLEX_G3 >> 18);
    int32_t z1 = z0 - (int32_t)(k1 << 14) + (int32_t)(SIMPLEX_G3 >> 18);
    int32_t x2 = x0 - (int32_t)(i2 << 14) + (int32_t)((2 * SIMPLEX_G3) >> 18);
    int32_t y2 = y0 - (int32_t)(j2 << 14) + (int32_t)((2 * SIMPLEX_G3) >> 18);
    int32_t z2 = z0 - (int32_t)(k2 << 14) + (int32_t)((2 * SIMPLEX_G3) >> 18);
    int32_t x3 = x0 - SIMPLEX_ONE + (int32_t)((3 * SIMPLEX_G3) >> 18);
    int32_t y3 = y0 - SIMPLEX_ONE + (int32_t)((3 * SIMPLEX_G3) >> 18);
    int32_t z3 = z0 - SIMPLEX_ONE + (int32_t)((3 * SIMPLEX_G3) >> 18);

    int32_t sum = simplex_corner(simplex_perm(i + simplex_perm(j + simplex_perm(k))), x0, y0, z0);

    sum += simplex_corner(simplex_perm(i + i1 + simplex_perm(j + j1 + simplex_perm(k + k1))), x1, y1, z1);
    sum += simplex_corner(simplex_perm(i + i2 + simplex_perm(j + j2 + simplex_perm(k + k2))), x2, y2, z2);
    sum += simplex_corner(simplex_perm(i + 1 + simplex_perm(j + 1 + simplex_perm(k + 1))), x3, y3, z3);

    /* snoise16 returns (uint16_t)(((sum >> 8) * 16748) >> 16) + 0x8000; this is its high byte. */
    return (uint8_t)(((uint32_t)(sum >> 8) * SIMPLEX_OUTPUT_SCALE + 0x80000000u) >> 24);
}

/*
 * Kind 14 renderer: moves the noise field along its third axis by elapsed_ms * speed / 32
 * and blends the two colours per key by the noise value at (row, col) * scale.
 */
void rgb_effect_kind_simplex_noise(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    const RgbEffectSimplexNoise *params = &layer->cfg.effect.simplex_noise;
    rgb_effect_noise_state_t    *state  = &layer->state.noise;

    state->time += (uint32_t)((int32_t)(elapsed_ms * params->speed) >> 5);

    uint32_t key_step = (uint32_t)params->scale << 4; /* one key = scale / 256 of a lattice cell */
    uint32_t color_a  = rgb_unpack_color(params->color_a);
    uint32_t color_b  = rgb_unpack_color(params->color_b);

    for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            uint8_t  noise = simplex_noise3_high_byte(row * key_step, col * key_step, state->time);
            uint32_t color = rgb_blend_argb(color_a, color_b, noise);

            rgb_frame_set_led(frame, rgb_effect_key_led(row, col), (uint8_t)(color >> 16), (uint8_t)(color >> 8),
                              (uint8_t)color);
        }
    }
}
