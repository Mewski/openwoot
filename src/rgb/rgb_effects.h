/* Public functions of the effect unit (rgb_effects_*.c): layer state, key and travel input, renderers. */
#ifndef RGB_RGB_EFFECTS_H_
#define RGB_RGB_EFFECTS_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "proto/settings.pb.h"
#include "rgb/rgb_types.h"
#include "rgb/rgb_data.h"

void rgb_effects_reset_key_state(void);
void rgb_effect_copy_spacebar_cell(uint8_t *key_map, uint32_t row, uint32_t col);
void rgb_effects_handle_key_event(uint32_t row, uint32_t col, uint32_t event);
void rgb_init_polar_tables(void);
void rgb_effect_reset_slot_state(rgb_effect_layer_t *slot);
void rgb_effect_render_hue_cycle(rgb_frame_t *frame, uint32_t palette, uint32_t period, rgb_effect_hue_state_t *state, uint16_t elapsed_ms);
void rgb_effect_render_scanner(rgb_frame_t *frame, const RgbEffectScanner *params, rgb_effect_scanner_state_t *state, uint16_t elapsed_ms);
void rgb_effect_render_level_meter(rgb_frame_t *frame, uint32_t min_scale, rgb_effect_level_meter_state_t *state);
void rgb_effect_render_breathing(rgb_frame_t *frame, uint32_t step_ms, uint32_t min_scale, rgb_effect_breathing_state_t *state, uint16_t elapsed_ms);
void rgb_effects_feed_key_travel(uint8_t row, uint8_t col, uint16_t travel);
void rgb_settings_convert_legacy_effect(void);
void rgb_effects_apply_layer_settings(void);

#endif /* RGB_RGB_EFFECTS_H_ */
