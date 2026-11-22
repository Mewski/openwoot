/*
 * layout_keymap.c - keymap layers: factory keymaps, copying a layer into the per-key records and
 * loading all layers of a profile from storage.
 */
#include <stdio.h>
#include <string.h>

#include "hal/hal_irq.h"

#include "layout/layout.h"
#include "analog/analog_data.h"
#include "profile/profile_data.h"
#include "rgb/rgb_data.h"
#include "rgb/rgb_frame.h"
#include "storage/storage.h"
#include "storage/storage_data.h"
#include "tables/layout_keycode_tables.h"
#include "tables/layout_tables.h"

/* Write the factory keymap of `layer` (126 key codes) to `keymap`: the board's default layer
 * (k_layout_default_keymap_layers), layer 0 with the key codes of the physical variant patched in;
 * all zero for a layer the board has no default for. Returns `keymap`. */
void *layout_get_default_keymap(uint32_t layer, uint8_t *keymap)
{
    uint32_t number;

    if (layer >= BOARD_DEFAULT_KEYMAP_LAYERS) {
        return memset(keymap, 0, LAYOUT_KEYMAP_SIZE);
    }
    memcpy(keymap, k_layout_default_keymap_layers[layer], LAYOUT_KEYMAP_SIZE);
    number = g_layout_physical_variant;
    if (layer == 0u && number < BOARD_LAYOUT_VARIANT_COUNT) {
        const layout_variant_t *variant = &k_layout_variants[number];

        for (uint32_t i = 0u; i < variant->layer0_patch_count; i++) {
            const layout_keymap_patch_t *patch = &variant->layer0_patches[i];

            keymap[patch->row * LAYOUT_KEYMAP_COLS + patch->col] = patch->key_code;
        }
    }
    return keymap;
}

/* Remember the position (row << 5 | col) of the first key bound to Fn (layer 1), Caps Lock,
 * Scroll Lock or Num Lock, so that the RGB code can light it as an indicator. */
void layout_record_indicator_key(uint32_t key_code, uint32_t row, uint32_t col)
{
    uint8_t key_pos = (uint8_t)(col | (row << 5));

    switch (key_code) {
    case LAYOUT_KEYCODE_LAYER1:
        if (g_rgb_layer_lock_key_pos == LAYOUT_NO_KEY) {
            g_rgb_layer_lock_key_pos = key_pos;
        }
        break;
    case LAYOUT_KEYCODE_CAPS_LOCK:
        if (g_rgb_caps_lock_key_pos == LAYOUT_NO_KEY) {
            g_rgb_caps_lock_key_pos = key_pos;
        }
        break;
    case LAYOUT_KEYCODE_SCROLL_LOCK:
        if (g_rgb_scroll_lock_key_pos == LAYOUT_NO_KEY) {
            g_rgb_scroll_lock_key_pos = key_pos;
        }
        break;
    case LAYOUT_KEYCODE_NUM_LOCK:
        if (g_rgb_num_lock_key_pos == LAYOUT_NO_KEY) {
            g_rgb_num_lock_key_pos = key_pos;
        }
        break;
    default:
        break;
    }
}

/* Copy one keymap layer (6 rows of 21 key codes) into the per-key records. Layer 0 also rebuilds
 * the indicator key positions and refreshes the lock indicators. */
void layout_apply_keymap_layer(const uint8_t *keymap, uint32_t layer)
{
    if (layer == 0u) {
        g_rgb_layer_lock_key_pos = k_layout_no_key_pos;
        g_rgb_caps_lock_key_pos = k_layout_no_key_pos;
        g_rgb_scroll_lock_key_pos = k_layout_no_key_pos;
        g_rgb_num_lock_key_pos = k_layout_no_key_pos;
    }

    for (uint32_t row = 0u; row < LAYOUT_ROWS; row++) {
        for (uint32_t col = 0u; col < LAYOUT_COLS; col++) {
            uint32_t scan_pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];

            if (scan_pos != LAYOUT_NO_KEY && layer <= 3u) {
                uint8_t key_code = keymap[col];

                layout_record_indicator_key(key_code, (uint8_t)row, (uint8_t)col);
                g_analog_key_states[scan_pos].keycode[layer] = key_code;
            }
        }
        keymap += LAYOUT_KEYMAP_COLS;
    }

    if (layer == 0u) {
        rgb_update_lock_indicators();
    }
}

/* Directory of a profile ("<index>" or "lp/<index>", empty for an invalid namespace). */
static void layout_format_profile_dir(char *dir, uint32_t ns, uint32_t index)
{
    if (ns == PROFILE_NS_BUILTIN) {
        sprintf(dir, "%d", (int)index);
    } else if (ns == PROFILE_NS_LP) {
        sprintf(dir, "lp/%d", (int)index);
    } else {
        dir[0] = '\0';
    }
}

/* End the RGB overlay with this id; same as rgb_overlay_stop without its id != 0 test. */
static void layout_stop_key_overlay(uint32_t overlay_id)
{
    uint32_t irq_state = hal_irq_save();

    for (uint8_t slot = 0u; slot < g_rgb_overlay_count; slot++) {
        if (g_rgb_overlay_slots[slot].id == overlay_id) {
            g_rgb_overlay_slots[slot].duration_ms = 0u;
            break;
        }
    }
    hal_irq_restore(irq_state);
}

/* Load all keymap layers of a profile from storage, with fallbacks to built-in profile 0 and the
 * factory keymaps, update the layer bits of g_layout_layer_state, then reset the press state of
 * every key and lift the "key disabled" toggles. */
void layout_load_keymap_layers(uint16_t profile_id)
{
    char dir[8];
    char name[28];       /* layer file name while counting, complete path while loading */
    char buffer[132];    /* complete path while counting, file name and keymap while loading */
    profile_id_t profile;
    uint32_t ns;
    uint32_t index;
    uint32_t layer_count;
    bool load_layers = true;

    g_layout_layer_state &= (uint8_t)~(LAYOUT_STATE_TOP_LAYER_MASK | LAYOUT_STATE_ACTIVE_MASK | LAYOUT_STATE_LOCKED_MASK);
    for (uint32_t pos = 0u; pos < ANALOG_KEY_COUNT; pos++) {
        memset(g_analog_key_states[pos].keycode, 0, sizeof(g_analog_key_states[pos].keycode));
    }

    /* Layer 0: the profile's own file, else the one of built-in profile 0, else the factory keymap. */
    profile.raw = profile_id;
    if (storage_read_keymap_layer(profile.raw, buffer, 0u) == 1) {
        g_layout_layer_state |= LAYOUT_STATE_OWN_KEYMAP;
        layout_apply_keymap_layer((const uint8_t *)buffer, 0u);
    } else {
        g_layout_layer_state &= (uint8_t)~LAYOUT_STATE_OWN_KEYMAP;
        if ((profile.index | profile.ns) == 0u || storage_read_keymap_layer(0u, buffer, 0u) != 1) {
            layout_get_default_keymap(0u, (uint8_t *)buffer);
        }
        layout_apply_keymap_layer((const uint8_t *)buffer, 0u);
    }

    /* Further layers come from the active profile (not from the profile_id argument) when it has
     * its own keymap, else from built-in profile 0. */
    if ((g_layout_layer_state & LAYOUT_STATE_OWN_KEYMAP) != 0u) {
        index = g_profile_active_id.index;
        ns = g_profile_active_id.ns;
    } else {
        index = 0u;
        ns = 0u;
    }

    if (g_storage_mounted != 0u) {
        /* Count the consecutive files ml0..ml3. */
        uint32_t present = 0u;

        while (present < LAYOUT_LAYERS) {
            sprintf(name, "ml%d", (int)present);
            layout_format_profile_dir(dir, ns, index);
            sprintf(buffer, "%s/%s", dir, name);
            if (storage_file_exists(buffer) == 0) {
                break;
            }
            present++;
        }

        if (present == LAYOUT_LAYERS) {
            g_layout_layer_state |= LAYOUT_STATE_TOP_LAYER_MASK;
            layer_count = LAYOUT_LAYERS;
        } else if (present == 0u) {
            /* No stored key map: the board's default layers are in use. */
            g_layout_layer_state = (uint8_t)((g_layout_layer_state & ~LAYOUT_STATE_TOP_LAYER_MASK) |
                                             (BOARD_DEFAULT_KEYMAP_LAYERS - 1u));
            layer_count = BOARD_DEFAULT_KEYMAP_LAYERS;
        } else {
            g_layout_layer_state =
                (uint8_t)((g_layout_layer_state & ~LAYOUT_STATE_TOP_LAYER_MASK) | ((present - 1u) & 3u));
            layer_count = present;
            if (present == 1u) {
                load_layers = false; /* only layer 0 exists */
            }
        }
    } else {
        g_layout_layer_state = (uint8_t)((g_layout_layer_state & ~LAYOUT_STATE_TOP_LAYER_MASK) |
                                         (BOARD_DEFAULT_KEYMAP_LAYERS - 1u));
        layer_count = BOARD_DEFAULT_KEYMAP_LAYERS;
    }

    if (load_layers) {
        uint32_t layer = 1u;

        do {
            bool loaded = false;

            if (g_storage_mounted != 0u) {
                int result;

                sprintf(buffer, "ml%d", (int)layer);
                layout_format_profile_dir(dir, ns, index);
                sprintf(name, "%s/%s", dir, buffer);
                result = storage_read_file(name, buffer, LAYOUT_KEYMAP_SIZE);
                loaded = !(result < 0 && (int16_t)result != 1);
            }
            if (!loaded) {
                layout_get_default_keymap(layer, (uint8_t *)buffer);
            }
            layout_apply_keymap_layer((const uint8_t *)buffer, layer);
            layer = (uint8_t)(layer + 1u);
        } while (layer < layer_count);
    }

    /* Reset the press state of every key and lift the "key disabled" toggles. */
    for (uint32_t pos = 0u; pos < ANALOG_KEY_COUNT; pos++) {
        analog_key_state_t *key = &g_analog_key_states[pos];

        key->flags &= (uint8_t)~(ANALOG_KEY_FLAG_RT_ENGAGED | ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED);
        key->press_ref_travel = 0u;
        if ((key->flags & ANALOG_KEY_FLAG_DISABLED) != 0u) {
            uint32_t key_id = g_layout_scanpos_to_keyid[pos];

            if (key_id != 0u) {
                layout_stop_key_overlay(key_id);
            }
            key->flags &= (uint8_t)~ANALOG_KEY_FLAG_DISABLED;
        }
    }
    rgb_update_lock_indicators();
}
