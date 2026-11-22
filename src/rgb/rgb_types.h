/*
 * rgb_types.h - data model of the rgb module: key geometry, colours, LED frame, effect layers.
 * The overlay records are in rgb_overlay.h; the optional features keep their own types.
 *
 * The layouts match stock firmware 2.14.1 of each board: the stored settings, the host
 * protocol and the code of this module depend on the offsets asserted below; the sizes follow
 * from the key matrix and the frame layout in board.h and are asserted as formulas over them.
 *
 * Protobuf-backed structs are NOT redeclared here: the generated nanopb types of
 * src/proto/settings.pb.h are used.
 *
 * Depends on board.h, rgb/rgb_frame_layout.h and proto/settings.pb.h.
 */
#ifndef RGB_TYPES_H
#define RGB_TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "board.h"
#include "proto/settings.pb.h"
#include "rgb/rgb_frame_layout.h"

/* ------------------------------------------------------------------------------------------
 * Geometry
 * ------------------------------------------------------------------------------------------ */

#define RGB_MATRIX_ROWS            BOARD_KEY_ROWS      /* key matrix rows */
#define RGB_MATRIX_COLS            BOARD_KEY_COLS      /* key matrix columns */
#define RGB_KEY_COUNT              BOARD_KEY_POSITIONS /* RGB_MATRIX_ROWS * RGB_MATRIX_COLS, size of the key -> LED tables */
#define RGB_BASE_COLOR_COLS        BOARD_KEYMAP_COLS   /* row stride of g_rgb_base_colors / file `clr` (2 bytes per column) */
#define RGB_COLOR_LAYER_COUNT      3    /* Fn-layer colour maps r1..r3 (g_rgb_layer_colors) */
#define RGB_FIRST_KEY_ROW          BOARD_KEY_FIRST_ROW /* first matrix row that carries keys (scanner effect) */

#define RGB_LEVEL_METER_SEGMENTS   BOARD_RGB_LEVEL_METER_SEGMENTS /* keys of the level meter effect */

/* Key position byte used by overlays and lock indicators: col | row << 5; 0xFF = none. */
#define RGB_KEY_POS(row, col)      ((uint8_t)(((row) << 5) | ((col) & 0x1F)))
#define RGB_KEY_POS_ROW(pos)       ((uint8_t)((pos) >> 5))
#define RGB_KEY_POS_COL(pos)       ((uint8_t)((pos) & 0x1F))
#define RGB_KEY_POS_NONE           0xFF

/* ------------------------------------------------------------------------------------------
 * Colours
 * ------------------------------------------------------------------------------------------ */

/*
 * Unpacked colour, 0xAARRGGBB in a register (byte 0 = blue). Return type of rgb_unpack_color
 * and rgb_convert_rgb565_to_argb, argument and return type of rgb_blend_argb.
 */
typedef struct {
    uint8_t b; /* 0x00 */
    uint8_t g; /* 0x01 */
    uint8_t r; /* 0x02 */
    uint8_t a; /* 0x03 alpha; 0xFF for RGB565 sources */
} rgb_argb_t;

_Static_assert(sizeof(rgb_argb_t) == 4, "rgb_argb_t size");
_Static_assert(offsetof(rgb_argb_t, r) == 2, "rgb_argb_t.r");
_Static_assert(offsetof(rgb_argb_t, a) == 3, "rgb_argb_t.a");

/*
 * Packed colour parameter (protobuf uint32 fields of effect kinds 6, 14, 15, the light-bar
 * effects and the indicator colours of the rgb settings), decoded by rgb_unpack_color:
 * bit 31 clear: bits 15:0 = RGB565, alpha 0xFF; bit 31 set: bits 23:0 = RGB888, bits 30:24 = alpha / 2.
 */
#define RGB_PACKED_COLOR_RGB888    0x80000000u
#define RGB_PACKED_COLOR_ALPHA(c)  ((uint8_t)((((c) >> 24) & 0x7F) << 1))

/* Seven-colour palette of effect kinds 5 and 6 (k_rgb_palette_red/green/blue, index = id - 1). */
#define RGB_PALETTE_COLOR_COUNT    7

/* ------------------------------------------------------------------------------------------
 * LED frame (g_rgb_frame_main and every `frame` argument): RGB_FRAME_PAGE_COUNT pages
 * ------------------------------------------------------------------------------------------ */

/*
 * Channel gains: rgb_frame_set_led stores (gain * (value + 1)) >> 8 for each channel, so a
 * gain of 255 keeps the full range and a smaller gain limits the channel.
 */
#define RGB_LED_GAIN_RED           BOARD_LED_GAIN_RED
#define RGB_LED_GAIN_GREEN         BOARD_LED_GAIN_GREEN
#define RGB_LED_GAIN_BLUE          BOARD_LED_GAIN_BLUE

/*
 * One frame page = one contiguous run of PWM registers of the LED driver (k_rgb_frame_pages).
 * The header is written once at start-up (rgb_frame_pages_init); the pwm bytes are written by
 * rgb_frame_set_led at the positions of k_rgb_led_channels.
 */
typedef struct {
    uint8_t index;                       /* 0x00 page number */
    uint8_t driver_page;                 /* 0x01 register bank of the LED driver */
    uint8_t first_reg;                   /* 0x02 first PWM register */
    uint8_t length;                      /* 0x03 number of PWM bytes sent */
    uint8_t pwm[RGB_FRAME_PAGE_PWM_MAX]; /* 0x04 linear channel values before gamma and brightness (see rgb_frame_locate_led) */
} rgb_frame_page_t;

/* Bytes of one frame page: the four header bytes and the PWM bytes of the longest page. */
#define RGB_FRAME_PAGE_SIZE        (4 + RGB_FRAME_PAGE_PWM_MAX)

_Static_assert(sizeof(rgb_frame_page_t) == RGB_FRAME_PAGE_SIZE, "rgb_frame_page_t size");
_Static_assert(offsetof(rgb_frame_page_t, driver_page) == 1, "rgb_frame_page_t.driver_page");
_Static_assert(offsetof(rgb_frame_page_t, first_reg) == 2, "rgb_frame_page_t.first_reg");
_Static_assert(offsetof(rgb_frame_page_t, length) == 3, "rgb_frame_page_t.length");
_Static_assert(offsetof(rgb_frame_page_t, pwm) == 4, "rgb_frame_page_t.pwm");

typedef struct {
    rgb_frame_page_t pages[RGB_FRAME_PAGE_COUNT]; /* 0x000 */
} rgb_frame_t;

/* rgb_update_frame clears a scratch frame as one block. */
_Static_assert(sizeof(rgb_frame_t) == RGB_FRAME_PAGE_COUNT * RGB_FRAME_PAGE_SIZE, "rgb_frame_t size");

/* ------------------------------------------------------------------------------------------
 * Queue of frame pages to send
 * ------------------------------------------------------------------------------------------ */

#define RGB_DIRTY_PAGE_QUEUE_LEN   RGB_FRAME_PAGE_COUNT /* g_rgb_dirty_pages: every page at most once */
#define RGB_DIRTY_PAGE_FREE        (-1) /* free queue entry (any negative value, see rgb_mark_page_dirty) */

/* ------------------------------------------------------------------------------------------
 * Frame timing, brightness, idle dimming
 * ------------------------------------------------------------------------------------------ */

#define RGB_FRAME_PERIOD_MS        16   /* rgb_update_frame renders when uptime - g_rgb_effect_stamp_ms > 15 */
#define RGB_BRIGHTNESS_STEP        5    /* rgb_step_brightness */
#define RGB_BRIGHTNESS_MAX         255
#define RGB_BRIGHTNESS_MAX_FAST_MODE 179 /* cap while g_run_state_flags bits 7:5 != 0 */
#define RGB_FADE_LEVEL_FULL        0xFF /* .data value of g_rgb_fade_level */
#define RGB_FADE_LEVEL_DIMMED      127  /* idle dim target (rgb_update_frame) */

/* g_rgb_idle_timer: time since the last key activity (counted by rgb_update_frame, cleared as one word by rgb_reset_idle_timer). */
typedef struct {
    uint16_t ms;      /* 0x00 0..999 */
    uint8_t  seconds; /* 0x02 0..60 */
    uint8_t  minutes; /* 0x03 stops counting at 100 */
} rgb_idle_timer_t;

_Static_assert(sizeof(rgb_idle_timer_t) == 4, "rgb_idle_timer_t size");
_Static_assert(offsetof(rgb_idle_timer_t, seconds) == 2, "rgb_idle_timer_t.seconds");
_Static_assert(offsetof(rgb_idle_timer_t, minutes) == 3, "rgb_idle_timer_t.minutes");

/* g_rgb_idle_flags */
#define RGB_IDLE_FLAG_DIMMED       0x01 /* idle dim fade (to 127) started */
#define RGB_IDLE_FLAG_OFF          0x02 /* idle off fade (to 0) started; reported by commands 73 and 0x4A */

/* ------------------------------------------------------------------------------------------
 * Effect layers (g_rgb_effect_layers): what a custom RGB effect plugs into
 * ------------------------------------------------------------------------------------------ */

#define RGB_EFFECT_LAYER_MAX       6

/*
 * Effect kind = oneof tag of the protobuf layer entry (RgbEffectLayer), uint16 at +0
 * of the run-time layer. The constants are the generated nanopb tag macros of that message
 * (<LayerType>_<member>_tag); they are not repeated here. Switches on the kind:
 * rgb_render_effect_layers (render), rgb_effect_reset_slot_state (state reset),
 * rgb_effects_apply_layer_settings (settings take-over), rgb_effects_feed_key_travel (analog
 * travel), rgb_effects_handle_key_event (key events), rgb_effects_reset_key_state (key state
 * reset), rgb_update_frame (dirty poll).
 *
 *  kind  member            state member  renderer
 *    1   rainbow wave      hue           rgb_effect_layers.c (rgb_effect_kind_rainbow_wave.c)
 *    2   scanner           scanner       rgb_effect_render_scanner
 *    3   breathing         breathing     rgb_effect_render_breathing
 *    4   hue cycle         hue           rgb_effect_render_hue_cycle
 *    5   key ripple        ripple        rgb_effect_kind_key_ripple.c, armed by key events
 *    6   reactive fade     reactive      rgb_render_reactive_fade_effect, armed by key events
 *    7   level meter       level_meter   rgb_effect_render_level_meter, fed by analog travel
 *    8   analog spotlight  key_map       rgb_effect_kind_analog.c, fed by analog travel
 *    9   analog glow       key_map       rgb_effect_kind_analog.c, fed by analog travel
 *   10   analog RGB fill   fill          rgb_frame_fill_keys, fed by analog travel
 *   11   key test          key_map       rgb_effect_kind_key_test.c, marks set by key events
 *   12   (unnamed)         none          no renderer, no state handling
 *   13   (unnamed, empty)  key_map       receives analog levels (rgb_effects_feed_key_travel), no renderer
 *   14   simplex noise     noise         rgb_effect_kind_simplex_noise.c
 *   15   polar gradient    polar         rgb_effect_kind_polar_gradient.c, needs g_rgb_key_distance_table / g_rgb_key_angle_table
 */

/* Key events fed into the layers by rgb_effects_handle_key_event. */
enum rgb_key_event {
    RGB_KEY_EVENT_PRESSED  = 0,
    RGB_KEY_EVENT_RELEASED = 1,
    RGB_KEY_EVENT_ACTUATED = 2, /* travelled past the actuation point */
    RGB_KEY_EVENT_RESET    = 3  /* back in the released zone */
};

/* Kinds 1 and 4: hue range selected by the preset parameter (rgb_effect_reset_slot_state). */
typedef struct {
    uint8_t  hue_a; /* 0x10 first hue of the range (presets: 110, 2, 64, 214, 113; else 0) */
    uint8_t  hue_b; /* 0x11 second hue of the range (presets: 184, 39, 114, 254, 204; else 255) */
    uint16_t phase; /* 0x12 16-bit phase accumulator, += 0x10000 / (250 * period) per ms; kind 1 keeps it on a parameter change */
} rgb_effect_hue_state_t;

/* Kind 2 (rgb_effect_render_scanner). */
typedef struct {
    int8_t   window_end;   /* 0x10 exclusive end of the bright window (init = width parameter) */
    int8_t   window_start; /* 0x11 first column / row of the window (init 0) */
    uint8_t  phase;        /* 0x12 0 left, 1 right (columns); 2 up, 3 down (rows); init = axis parameter */
    uint8_t  _pad_13;
    uint16_t timer_ms;     /* 0x14 elapsed time not yet turned into steps */
} rgb_effect_scanner_state_t;

/* Kind 3 (rgb_effect_render_breathing). */
typedef struct {
    uint8_t  level;    /* 0x10 0..250, step 2 */
    uint8_t  _pad_11;
    uint16_t timer_ms; /* 0x12 elapsed time not yet turned into steps */
    uint8_t  falling;  /* 0x14 0 rising, 1 falling */
} rgb_effect_breathing_state_t;

/*
 * Kind 5 (key ripple, armed by rgb_effects_handle_key_event). Cell byte: bits 2:0 colour (0 = empty,
 * 1..7 = palette index + 1), bit 3 tail, bits 7:4 head: propagation code 0..5 / tail: level 15..0.
 */
typedef struct {
    uint8_t  cells[RGB_MATRIX_ROWS][RGB_MATRIX_COLS]; /* 0x10 ripple automaton grid */
    uint8_t  last_color;     /* last random palette index 1..7 (rgb_effects_handle_key_event) */
    uint8_t  active;         /* 1 while any cell is non-empty; dirty flag polled by rgb_update_frame */
    uint16_t move_timer_ms;  /* ms accumulator of the move step */
    uint16_t decay_timer_ms; /* ms accumulator of the decay step */
} rgb_effect_ripple_state_t;

#define RGB_RIPPLE_CELL_COLOR_MASK 0x07
#define RGB_RIPPLE_CELL_TAIL       0x08
#define RGB_RIPPLE_CELL_LEVEL_SHIFT 4

/*
 * Kind 6 (rgb_render_reactive_fade_effect, armed by rgb_effects_handle_key_event). Cell byte,
 * plain / fixed-colour mode: 0 idle, 251 just pressed, 250..1 counting down. Palette mode
 * (parameter flags bit 0): bits 7:3 level 31..1, bits 2:0 palette index 1..7.
 */
typedef struct {
    uint8_t  cells[RGB_MATRIX_ROWS][RGB_MATRIX_COLS]; /* 0x10 per-key countdown */
    uint8_t  last_color; /* last random palette index 1..7 (rgb_effects_handle_key_event) */
    uint8_t  active;     /* 1 while any key is animating; dirty flag polled by rgb_update_frame */
    uint16_t timer_ms;   /* elapsed time not yet turned into steps */
} rgb_effect_reactive_state_t;

#define RGB_REACTIVE_CELL_PRESSED  251

/* Kind 7 (rgb_effect_render_level_meter, fed by rgb_effects_feed_key_travel). */
typedef struct {
    uint16_t level; /* 0x10 travel of the deepest key, 0..4095 */
    uint8_t  row;   /* 0x12 matrix row of that key, 0xFF = none */
    uint8_t  col;   /* 0x13 matrix column of that key, 0xFF = none */
    uint8_t  dirty; /* 0x14 set on every level change, cleared by the renderer; polled by rgb_update_frame */
} rgb_effect_level_meter_state_t;

/*
 * Kinds 8, 9, 13: per-key analog level 0..255 (rgb_effects_feed_key_travel). Kind 11: per-key
 * mark 0 idle, 1 red, 2 green (rgb_effects_handle_key_event).
 */
typedef struct {
    uint8_t cells[RGB_MATRIX_ROWS][RGB_MATRIX_COLS]; /* 0x10 */
    uint8_t dirty; /* set when a cell changes, cleared after drawing; polled by rgb_update_frame (kinds 8, 9, 11) */
} rgb_effect_key_map_state_t;

/* Kind 10: levels of the last three keys of the bottom row (rgb_effects_feed_key_travel). */
typedef struct {
    uint8_t red;   /* 0x10 */
    uint8_t green; /* 0x11 */
    uint8_t blue;  /* 0x12 */
    uint8_t dirty; /* 0x13 initialised to 1 and set on every change; never cleared, so the layer renders every frame */
} rgb_effect_fill_state_t;

/* Kind 14 (simplex noise). */
typedef struct {
    uint32_t time; /* 0x10 noise z coordinate, += (elapsed_ms * speed) >> 5 per frame */
} rgb_effect_noise_state_t;

/* Kind 15 (polar gradient). */
typedef struct {
    uint16_t phase; /* 0x10 rotation phase, += elapsed_ms * speed, kept modulo 65535 */
} rgb_effect_polar_state_t;

/*
 * Animation state of one layer, zeroed by rgb_effects_apply_layer_settings and initialised by
 * rgb_effect_reset_slot_state. Its size follows from the largest member, the ripple state:
 * the per-key cell map plus 6 bytes, rounded up to a multiple of 4.
 */
typedef union {
    rgb_effect_hue_state_t         hue;         /* kinds 1, 4 */
    rgb_effect_scanner_state_t     scanner;     /* kind 2 */
    rgb_effect_breathing_state_t   breathing;   /* kind 3 */
    rgb_effect_ripple_state_t      ripple;      /* kind 5 */
    rgb_effect_reactive_state_t    reactive;    /* kind 6 */
    rgb_effect_level_meter_state_t level_meter; /* kind 7 */
    rgb_effect_key_map_state_t     key_map;     /* kinds 8, 9, 11, 13 */
    rgb_effect_fill_state_t        fill;        /* kind 10 */
    rgb_effect_noise_state_t       noise;       /* kind 14 */
    rgb_effect_polar_state_t       polar;       /* kind 15 */
} rgb_effect_state_t;

/*
 * Run-time effect layer. cfg is a verbatim copy of the 16-byte layer entry of the rgb
 * settings: uint16 oneof tag (= effect kind, see the table above) at +0, per-kind parameter
 * sub-message at +4..+0xF (copied by rgb_effects_apply_layer_settings).
 */
typedef struct {
    RgbEffectLayer     cfg;   /* 0x00 layer entry of the rgb settings */
    rgb_effect_state_t state; /* 0x10 */
} rgb_effect_layer_t;

/* Layer offset of the first byte behind the per-key cell map. */
#define RGB_EFFECT_CELLS_END       (0x10 + RGB_KEY_COUNT)

/* Size of rgb_effect_state_t: the per-key cell map and the 6 bytes behind it of the ripple
 * state, rounded up to a multiple of 4. */
#define RGB_EFFECT_STATE_SIZE      ((RGB_KEY_COUNT + 6 + 3) / 4 * 4)

_Static_assert(sizeof(RgbEffectLayer) == 16, "effect layer entry size");
_Static_assert(sizeof(rgb_effect_state_t) == RGB_EFFECT_STATE_SIZE, "rgb_effect_state_t size");
_Static_assert(sizeof(rgb_effect_layer_t) == 0x10 + RGB_EFFECT_STATE_SIZE, "rgb_effect_layer_t size");
_Static_assert(offsetof(rgb_effect_layer_t, state) == 0x10, "rgb_effect_layer_t.state");
_Static_assert(offsetof(rgb_effect_layer_t, state.hue.hue_b) == 0x11, "hue.hue_b");
_Static_assert(offsetof(rgb_effect_layer_t, state.hue.phase) == 0x12, "hue.phase");
_Static_assert(offsetof(rgb_effect_layer_t, state.scanner.window_start) == 0x11, "scanner.window_start");
_Static_assert(offsetof(rgb_effect_layer_t, state.scanner.phase) == 0x12, "scanner.phase");
_Static_assert(offsetof(rgb_effect_layer_t, state.scanner.timer_ms) == 0x14, "scanner.timer_ms");
_Static_assert(offsetof(rgb_effect_layer_t, state.breathing.timer_ms) == 0x12, "breathing.timer_ms");
_Static_assert(offsetof(rgb_effect_layer_t, state.breathing.falling) == 0x14, "breathing.falling");
_Static_assert(offsetof(rgb_effect_layer_t, state.ripple.cells) == 0x10, "ripple.cells");
_Static_assert(offsetof(rgb_effect_layer_t, state.ripple.last_color) == RGB_EFFECT_CELLS_END, "ripple.last_color");
_Static_assert(offsetof(rgb_effect_layer_t, state.ripple.active) == RGB_EFFECT_CELLS_END + 1, "ripple.active");
_Static_assert(offsetof(rgb_effect_layer_t, state.ripple.move_timer_ms) == RGB_EFFECT_CELLS_END + 2, "ripple.move_timer_ms");
_Static_assert(offsetof(rgb_effect_layer_t, state.ripple.decay_timer_ms) == RGB_EFFECT_CELLS_END + 4, "ripple.decay_timer_ms");
_Static_assert(offsetof(rgb_effect_layer_t, state.reactive.last_color) == RGB_EFFECT_CELLS_END, "reactive.last_color");
_Static_assert(offsetof(rgb_effect_layer_t, state.reactive.active) == RGB_EFFECT_CELLS_END + 1, "reactive.active");
_Static_assert(offsetof(rgb_effect_layer_t, state.reactive.timer_ms) == RGB_EFFECT_CELLS_END + 2, "reactive.timer_ms");
_Static_assert(offsetof(rgb_effect_layer_t, state.level_meter.row) == 0x12, "level_meter.row");
_Static_assert(offsetof(rgb_effect_layer_t, state.level_meter.col) == 0x13, "level_meter.col");
_Static_assert(offsetof(rgb_effect_layer_t, state.level_meter.dirty) == 0x14, "level_meter.dirty");
_Static_assert(offsetof(rgb_effect_layer_t, state.key_map.dirty) == RGB_EFFECT_CELLS_END, "key_map.dirty");
_Static_assert(offsetof(rgb_effect_layer_t, state.fill.green) == 0x11, "fill.green");
_Static_assert(offsetof(rgb_effect_layer_t, state.fill.blue) == 0x12, "fill.blue");
_Static_assert(offsetof(rgb_effect_layer_t, state.fill.dirty) == 0x13, "fill.dirty");
_Static_assert(offsetof(rgb_effect_layer_t, state.noise.time) == 0x10, "noise.time");
_Static_assert(offsetof(rgb_effect_layer_t, state.polar.phase) == 0x10, "polar.phase");

/* ------------------------------------------------------------------------------------------
 * Size of the protobuf-backed settings (generated type; the stored file depends on it)
 * ------------------------------------------------------------------------------------------ */

_Static_assert(sizeof(RgbSettings) == 132, "rgb settings size");

#endif /* RGB_TYPES_H */
