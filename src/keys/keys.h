/* Public functions of the keys module: applying and sanitising the per-key settings of a profile
 * (actuation points, rapid trigger, gamepad bindings, advanced keys) and the per-key processing
 * steps used by the scan pass. */
#ifndef KEYS_KEYS_H_
#define KEYS_KEYS_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "proto/settings.pb.h"
#include "keys/keys_types.h"
#include "keys/keys_data.h"

void keys_apply_actuation_map(const ActuationPoints *act);
void keys_apply_gamepad_map(const GamepadMapping *gm);
void keys_sanitize_distance(uint16_t *value, uint32_t min_q12, uint32_t default_q12);
void keys_sanitize_actuation_map(ActuationPoints *act);
void keys_modtap_release_slot(int slot);
uint32_t keys_get_binding_layer(const keys_key_ref_t *key_ref);
void keys_dks_queue_event(keys_dks_binding_queue_t *binding_queue, uint32_t event);
void keys_dks_queue_events(keys_advanced_key_state_t *dks_state, uint32_t event_mask);
void keys_apply_key_action(keys_pass_ctx_t *ctx, uint16_t action, uint8_t key_id, uint8_t layer, uint16_t analog, uint8_t force_keyboard, uint8_t pressed);
void keys_process_advanced_keys(keys_pass_ctx_t *ctx, keys_key_ref_t *key_ref, uint32_t analog, uint32_t current_index);
void keys_sanitize_rapid_trigger_groups(RapidTriggerGroups *rt_groups);
void keys_apply_rapid_trigger_groups(void);
void keys_apply_legacy_rapid_trigger_keys(const KeyByteMap *prt_msg);
uint32_t keys_get_actuation_threshold(keys_key_ref_t *key_ctx);
void keys_update_press_state(keys_key_ref_t *key_ctx, uint32_t travel);
void keys_validate_advanced_keys(AdvancedKeys *ak_msg);
void keys_stop_toggle_overlays(void);
void keys_apply_advanced_key_entry(uint8_t ak_index, const AdvancedKey *entry);
void keys_bind_advanced_key(uint32_t row, uint32_t col, uint32_t layer, uint8_t ak_index);

#endif /* KEYS_KEYS_H_ */
