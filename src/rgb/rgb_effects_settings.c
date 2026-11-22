/*
 * rgb_effects_settings.c - conversion of the legacy single-effect form of the rgb settings
 * (RgbSettings oneof tag 3) into the effect layer list (tag 4).
 */
#include "rgb/rgb_effects.h"

#include <string.h>

/* Legacy base level when the exact value is absent: 38 per index step (modulo 256). */
#define RGB_LEGACY_BASE_LEVEL_STEP 38

/* Key ripple: legacy base levels above this value switch the effect to inverted levels. */
#define RGB_LEGACY_RIPPLE_INVERT_ABOVE 170
#define RGB_RIPPLE_FLAG_PALETTE        0x01 /* RgbEffectKeyRipple.flags bit 0 */
#define RGB_RIPPLE_FLAG_INVERTED       0x04 /* RgbEffectKeyRipple.flags bit 2 */

/* Legacy speed setting to the timing parameter of an effect; unknown settings give 0. */
static uint8_t rgb_legacy_speed_to_param(RgbLegacySpeed speed, uint8_t slow, uint8_t medium, uint8_t fast)
{
    switch (speed) {
    case RgbLegacySpeed_RGB_LEGACY_SPEED_SLOW:
        return slow;
    case RgbLegacySpeed_RGB_LEGACY_SPEED_MEDIUM:
        return medium;
    case RgbLegacySpeed_RGB_LEGACY_SPEED_FAST:
        return fast;
    default:
        return 0;
    }
}

/*
 * Replaces the legacy single-effect form of g_rgb_settings (oneof tag 3) by a layer list
 * (tag 4) with one equivalent layer, or an empty list when the legacy effect is "none".
 */
void rgb_settings_convert_legacy_effect(void)
{
    /* The layer list overlays the legacy message inside g_rgb_settings: take a copy first. */
    const RgbLegacyEffect legacy = g_rgb_settings.effect.legacy_effect;
    uint8_t base_level = legacy.has_base_level ? legacy.base_level
                                               : (uint8_t)(RGB_LEGACY_BASE_LEVEL_STEP * legacy.base_level_index);
    bool use_gamepad_curve = legacy.f6 == 0;
    RgbEffectLayer layer;

    if (legacy.effect == RgbEffectKind_RGB_EFFECT_KIND_NONE) {
        g_rgb_settings.effect.layers.layers_count = 0;
        g_rgb_settings.which_effect = RgbSettings_layers_tag;
        return;
    }

    memset(&layer, 0, sizeof(layer));

    switch (legacy.effect) {
    case RgbEffectKind_RGB_EFFECT_KIND_RAINBOW_WAVE:
        layer.which_effect = RgbEffectLayer_rainbow_wave_tag;
        layer.effect.rainbow_wave.hue_preset = (RgbHuePreset)legacy.color_mode;
        layer.effect.rainbow_wave.period_250ms = rgb_legacy_speed_to_param(legacy.speed, 40, 16, 8);
        layer.effect.rainbow_wave.direction = legacy.f6;
        break;

    case RgbEffectKind_RGB_EFFECT_KIND_SCANNER:
        layer.which_effect = RgbEffectLayer_scanner_tag;
        layer.effect.scanner.width = legacy.scanner_width;
        layer.effect.scanner.steps_per_second = rgb_legacy_speed_to_param(legacy.speed, 11, 15, 31);
        layer.effect.scanner.outside_level = base_level;
        layer.effect.scanner.start_direction = RgbScannerDirection_RGB_SCANNER_DIRECTION_LEFT;
        break;

    case RgbEffectKind_RGB_EFFECT_KIND_BREATHING:
        layer.which_effect = RgbEffectLayer_breathing_tag;
        layer.effect.breathing.step_ms = rgb_legacy_speed_to_param(legacy.speed, 30, 15, 12);
        layer.effect.breathing.min_level = base_level;
        break;

    case RgbEffectKind_RGB_EFFECT_KIND_HUE_CYCLE:
        layer.which_effect = RgbEffectLayer_hue_cycle_tag;
        layer.effect.hue_cycle.hue_preset = (RgbHuePreset)legacy.color_mode;
        layer.effect.hue_cycle.period_250ms = rgb_legacy_speed_to_param(legacy.speed, 62, 48, 32);
        break;

    case RgbEffectKind_RGB_EFFECT_KIND_KEY_RIPPLE:
        layer.which_effect = RgbEffectLayer_key_ripple_tag;
        layer.effect.key_ripple.flags = (legacy.color_mode != 0) ? RGB_RIPPLE_FLAG_PALETTE : 0;
        layer.effect.key_ripple.steps_per_second = 20;
        if (base_level <= RGB_LEGACY_RIPPLE_INVERT_ABOVE) {
            layer.effect.key_ripple.decay_step_ms = 30;
            layer.effect.key_ripple.base_level = base_level;
        } else {
            layer.effect.key_ripple.flags |= RGB_RIPPLE_FLAG_INVERTED;
            layer.effect.key_ripple.decay_step_ms = 16;
            layer.effect.key_ripple.base_level = 80;
        }
        break;

    case RgbEffectKind_RGB_EFFECT_KIND_REACTIVE_FADE:
        layer.which_effect = RgbEffectLayer_reactive_fade_tag;
        layer.effect.reactive_fade.base_level = base_level;
        layer.effect.reactive_fade.step_ms = rgb_legacy_speed_to_param(legacy.speed, 8, 4, 2);
        layer.effect.reactive_fade.flags = legacy.color_mode == 1; /* bit 0: random palette colours */
        break;

    case RgbEffectKind_RGB_EFFECT_KIND_LEVEL_METER:
        layer.which_effect = RgbEffectLayer_level_meter_tag;
        layer.effect.level_meter.base_level = base_level;
        layer.effect.level_meter.use_gamepad_curve = use_gamepad_curve;
        break;

    case RgbEffectKind_RGB_EFFECT_KIND_ANALOG_SPOTLIGHT:
        layer.which_effect = RgbEffectLayer_analog_spotlight_tag;
        layer.effect.analog_spotlight.base_level = base_level;
        layer.effect.analog_spotlight.use_gamepad_curve = use_gamepad_curve;
        break;

    case RgbEffectKind_RGB_EFFECT_KIND_ANALOG_GLOW:
        layer.which_effect = RgbEffectLayer_analog_glow_tag;
        layer.effect.analog_glow.base_level = base_level;
        layer.effect.analog_glow.use_gamepad_curve = use_gamepad_curve;
        break;

    case RgbEffectKind_RGB_EFFECT_KIND_ANALOG_RGB_FILL:
        layer.which_effect = RgbEffectLayer_analog_rgb_fill_tag;
        layer.effect.analog_rgb_fill.use_gamepad_curve = use_gamepad_curve;
        break;

    case RgbEffectKind_RGB_EFFECT_KIND_KEY_TEST:
        layer.which_effect = RgbEffectLayer_key_test_tag;
        layer.effect.key_test.base_level = base_level;
        break;

    default:
        /* Unknown legacy effect: one empty layer of kind 0, which nothing draws. */
        break;
    }

    memcpy(&g_rgb_settings.effect.layers.layers[0], &layer, sizeof(layer));
    g_rgb_settings.effect.layers.layers_count = 1;
    g_rgb_settings.which_effect = RgbSettings_layers_tag;
}
