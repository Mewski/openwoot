/*
 * layout_types.h - types and constants of the `layout` module (key matrix geometry, physical
 * layout variants, keymap layers, key codes, special-key actions).
 *
 * Depends on board.h for the matrix dimensions. The board's tables/layout_tables.c uses the
 * variant types to describe its variants and names them.
 */
#ifndef LAYOUT_TYPES_H
#define LAYOUT_TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "board.h"

/* ---- Key matrix -------------------------------------------------------------------------------- */

#define LAYOUT_ROWS             ((unsigned)BOARD_KEY_ROWS)     /* matrix rows */
#define LAYOUT_COLS             ((unsigned)BOARD_KEY_COLS)     /* matrix columns with sensors; g_layout_keyid_to_scanpos is [row * LAYOUT_COLS + col] */
#define LAYOUT_KEYMAP_COLS      ((unsigned)BOARD_KEYMAP_COLS)  /* columns per row in keymap files / messages; those from LAYOUT_COLS on are unused */
#define LAYOUT_KEYMAP_SIZE      (LAYOUT_ROWS * LAYOUT_KEYMAP_COLS) /* key codes of one layer: files ml0..ml3, layout_get_default_keymap */
#define LAYOUT_LAYERS           4u    /* keymap layers 0..3 */
#define LAYOUT_SCAN_POSITIONS   ((unsigned)BOARD_SCAN_POSITIONS) /* entries of the g_layout_scanpos_to_keyid table */
#define LAYOUT_NO_KEY           0xFFu /* table entry of a position without key / sensor */

/* ---- Physical layout variants ------------------------------------------------------------------ */

/* A physical layout variant is known here only by its number: the value stored in the device
 * settings (g_layout_physical_variant) and the index of the board's k_layout_variants. A board has
 * the variants 0 .. BOARD_LAYOUT_VARIANT_COUNT - 1; its tables/layout_tables.h names them. */

/* One key code that a variant writes over the factory keymap of layer 0. */
typedef struct {
    uint8_t row;       /* keymap row */
    uint8_t col;       /* keymap column (0 .. LAYOUT_KEYMAP_COLS - 1) */
    uint8_t key_code;
} layout_keymap_patch_t;

/* The tables of one physical layout variant; layout_select_variant points the run-time lookups
 * at them. */
typedef struct {
    const uint8_t *keyid_to_scanpos;        /* [row * LAYOUT_COLS + col] -> scan position or LAYOUT_NO_KEY */
    const uint8_t *scanpos_to_keyid;        /* [scan position] -> key id (row << 5 | col) or LAYOUT_NO_KEY */
    const uint8_t *key_to_led;              /* [row * LAYOUT_COLS + col] -> LED number (board rgb tables) */
    const uint8_t *spacebar_fill_cols;      /* columns of the space-bar row that mirror the space-bar cell */
    uint8_t spacebar_fill_col_count;
    const layout_keymap_patch_t *layer0_patches; /* applied by layout_get_default_keymap; NULL if none */
    uint8_t layer0_patch_count;
} layout_variant_t;

/* ---- Layer state ------------------------------------------------------------------------------- */

/* g_layout_layer_state. */
#define LAYOUT_STATE_TOP_LAYER_MASK    0x03u  /* bits 1:0 highest keymap layer present for the profile */
#define LAYOUT_STATE_ACTIVE_MASK       0x0Cu  /* bits 3:2 active layer */
#define LAYOUT_STATE_ACTIVE_SHIFT      2u
#define LAYOUT_STATE_LOCKED_MASK       0x30u  /* bits 5:4 locked layer, 0 = none */
#define LAYOUT_STATE_LOCKED_SHIFT      4u
#define LAYOUT_STATE_OWN_KEYMAP        0x40u  /* bit 6 layer 0 (ml0) was loaded from the profile itself, else from profile 0 / factory keymap */

/* ---- Key codes --------------------------------------------------------------------------------- */

/* Key code = byte stored in keymaps and advanced keys = index into k_layout_keycode_actions
 * (256 x uint16_t). Only the codes the firmware tests by value are named here. */
#define LAYOUT_KEYCODE_NONE            0xFFu  /* no key; looked up as code 0 */
#define LAYOUT_KEYCODE_CAPS_LOCK       0x36u  /* 54 (mod-tap tap length 100 ms) */
#define LAYOUT_KEYCODE_SCROLL_LOCK     0x44u  /* 68 */
#define LAYOUT_KEYCODE_NUM_LOCK        0x50u  /* 80 */
#define LAYOUT_KEYCODE_MOD_FIRST       0x63u  /* 99..106: modifier bits 0..7 (LCtrl .. RGui) */
#define LAYOUT_KEYCODE_MOD_LAST        0x6Au
#define LAYOUT_KEYCODE_GAMEPAD_FIRST   0x78u  /* 120..155: gamepad controls (hid_gamepad_apply_binding) */
#define LAYOUT_KEYCODE_GAMEPAD_LAST    0x9Bu
#define LAYOUT_KEYCODE_LAYER1          0xE7u  /* 231 momentary layer 1 = Fn */
#define LAYOUT_KEYCODE_LAYER2          0xEAu  /* 234 momentary layer 2 */
#define LAYOUT_KEYCODE_LAYER3          0xEBu  /* 235 momentary layer 3 */

/* Action word = k_layout_keycode_actions[key code] (decoded by keys_apply_key_action). */
#define LAYOUT_ACTION_KIND_MASK        0x0007u /* bits 2:0 enum layout_action_kind */
#define LAYOUT_ACTION_ANALOG           0x0008u /* bit 3: apply on every pass with the analog value, even when not pressed */
#define LAYOUT_ACTION_CODE_SHIFT       8u      /* high byte: code (usage, modifier mask, consumer code, action number, gamepad control) */

enum layout_action_kind {
    LAYOUT_ACTION_KEYBOARD = 0,  /* code = HID keyboard usage (0 = no action) */
    LAYOUT_ACTION_MODIFIER = 1,  /* code = modifier bit mask */
    LAYOUT_ACTION_EXTRA    = 2,  /* code 1..3 system control, 4..11 button bits, else consumer usage */
    LAYOUT_ACTION_SPECIAL  = 3,  /* code = enum layout_key_action */
    LAYOUT_ACTION_GAMEPAD  = 4,  /* code = gamepad control 120..155 */
};

/* ---- Special-key (Fn) actions -------------------------------------------------------------------- */

/* Code of a LAYOUT_ACTION_SPECIAL action word = index into k_layout_key_action_handlers (slot 0 is
 * NULL). Key codes 0xE0..0xE5 map to actions 1..6, 0xE6..0xEC to actions 8..14. */
enum layout_key_action {
    LAYOUT_KEY_ACTION_INVALID          = 0,   /* NULL handler; no key code maps to it */
    LAYOUT_KEY_ACTION_BRIGHTNESS_UP    = 1,   /* auto-repeating brightness step up while held */
    LAYOUT_KEY_ACTION_BRIGHTNESS_DOWN  = 2,   /* auto-repeating brightness step down while held */
    LAYOUT_KEY_ACTION_PROFILE_1        = 3,   /* switch to built-in profile 1 */
    LAYOUT_KEY_ACTION_PROFILE_2        = 4,   /* switch to built-in profile 2 */
    LAYOUT_KEY_ACTION_PROFILE_3        = 5,   /* switch to built-in profile 3 */
    LAYOUT_KEY_ACTION_PROFILE_0        = 6,   /* switch to built-in profile 0 */
    LAYOUT_KEY_ACTION_NONE             = 7,   /* does nothing; no key code maps to it */
    LAYOUT_KEY_ACTION_TOGGLE_MODE_PROFILE = 8, /* toggle between profile 0 and the remembered profile */
    LAYOUT_KEY_ACTION_LAYER1           = 9,   /* momentary layer 1 */
    LAYOUT_KEY_ACTION_TOGGLE_KEY_DISABLE = 10, /* disable / enable the key at this position */
    LAYOUT_KEY_ACTION_LAYER_LOCK       = 11,  /* third argument = layer to lock */
    LAYOUT_KEY_ACTION_LAYER2           = 12,  /* momentary layer 2 */
    LAYOUT_KEY_ACTION_LAYER3           = 13,  /* momentary layer 3 */
    LAYOUT_KEY_ACTION_NEXT_PROFILE     = 14,  /* cycle to the next profile */
    LAYOUT_KEY_ACTION_COUNT            = 15,
};

/* Handler of a special-key action: row / col = position of the key, layer = layer of the binding
 * (bits 1:0 of the action state), pressed = new state. All handlers return 1. */
typedef int (*layout_key_action_handler_t)(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);

#endif /* LAYOUT_TYPES_H */
