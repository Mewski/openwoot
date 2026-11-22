/*
 * analog_types.h - types and constants of the `analog` module (per-key calibration, filtering,
 * sensor value to travel conversion, bottom-out learning, idle noise recalibration).
 *
 * The layouts are fixed by the _Static_assert lines that follow each type.
 *
 * The per-key record analog_key_state_t is the central object of the firmware: the ADC interrupt
 * (adc_scan), the key task (keys), the keymap code (layout), the profile loader and several
 * protocol commands all work on it. Its idle noise statistics are the record of the selected
 * noise_recal variant (noise_stats.h of that variant).
 */
#ifndef ANALOG_TYPES_H
#define ANALOG_TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "board.h"
#include "noise_stats.h"
#include "proto/settings.pb.h"

/* ---- Sizes and value ranges ---------------------------------------------------------------- */

#define ANALOG_KEY_COUNT             ((unsigned)BOARD_SCAN_POSITIONS) /* scan positions: multiplexers x positions per multiplexer */
#define ANALOG_KEY_LAYERS            4u      /* keymap layers per key (layer 0..3) */
#define ANALOG_NO_SCAN_POS           0xFFu   /* "no key here" in g_layout_keyid_to_scanpos / g_layout_scanpos_to_keyid */
#define ANALOG_NO_ADVANCED_KEY       0xFFu   /* analog_ak_slot_t.index: no advanced key bound */

#define ANALOG_ADC_DEADBAND          79u     /* a reading within +/-79 of the zero-field level reads as magnitude 0 */
#define ANALOG_VALUE_NONE            0x0FFFu /* "no value yet" marker of bo_reference / bo_estimate / bo_tracker.lower_bound / rebound.prev_reading */
#define ANALOG_REST_MARGIN           5u      /* key is at rest while value <= (uint16_t)(rest_level + 5) */
#define ANALOG_TRAVEL_MAX            4095u   /* run-time travel: 0 = released, 4095 = end of travel */
#define ANALOG_CURVE_END_Q8_MAX      0xFFF00u /* analog_switch_params_t.curve_end_q8 without travel offset = 4095 << 8 */
#define ANALOG_CURVE_VALUE_MAX       0x3FFFu /* largest value in k_analog_travel_curves */
#define ANALOG_SWITCH_TYPE_COUNT     49u     /* entries of k_analog_switch_travel and of the other per-switch-type tables */
#define ANALOG_MIN_BOTTOM_DISTANCE   50u     /* bottom_level is at least rest + 50 when derived from the nominal value */
#define ANALOG_BOTTOM_HEADROOM       240u    /* un-learned bottom_level follows the value at a distance of 240 */
#define ANALOG_BO_CONFIDENCE_ADOPT   149u    /* bo_tracker.confidence above this adopts bo_estimate as bottom_level */

#define ANALOG_RECAL_HOLD_MS         5000u   /* recalibration blocked this long after key activity */
#define ANALOG_RECAL_PASS_PAUSE_MS   20000u  /* pause between two complete recalibration passes */

#define ANALOG_REBOUND_REST_US       3500u   /* rebound filter rest timeout in microseconds (hal_time_micros) */

/* ---- Run-time switch parameters -------------------------------------------------------------- */

/* 16 bytes. g_analog_default_switch_params and analog_key_state_t.switch_params (+0x9C).
 * Built by analog_build_switch_params from the 6-byte switch description of file "sw"
 * (the first member is a plain copy of it). */
typedef struct analog_switch_params {
    SwitchParams sw;         /* +0x00 oneof tag (1 = switch type set), +0x02 switch type id 0..48, +0x03 has travel offset, +0x04 travel offset in 0.05 mm */
    uint32_t curve_end_q8;   /* +0x08 position in the travel curve that counts as end of travel, Q8 index; 0xFFF00 without travel offset */
    uint8_t  learn_strength; /* +0x0C bottom-out learning strength 0..255: per-type table value, scaled by (travel - offset) / travel; 0 disables the upper / lower bound logic */
    uint16_t nominal_bottom; /* +0x0E nominal bottom-out sensor level of the switch type: per-type table value, 700..1450 */
} analog_switch_params_t;

_Static_assert(sizeof(SwitchParams) == 6, "switch description size");
_Static_assert(sizeof(analog_switch_params_t) == 16, "analog_switch_params_t size");
_Static_assert(offsetof(analog_switch_params_t, sw) == 0x00, "analog_switch_params_t.sw");
_Static_assert(offsetof(analog_switch_params_t, curve_end_q8) == 0x08, "analog_switch_params_t.curve_end_q8");
_Static_assert(offsetof(analog_switch_params_t, learn_strength) == 0x0C, "analog_switch_params_t.learn_strength");
_Static_assert(offsetof(analog_switch_params_t, nominal_bottom) == 0x0E, "analog_switch_params_t.nominal_bottom");

/* ---- Bottom-out learning ---------------------------------------------------------------------- */

/* 60 bytes at analog_key_state_t +0x14. State of analog_update_bottom_out_estimate (ADC interrupt
 * context). Cleared as one block (memset of 0x3C bytes) by the tracker reset, by
 * analog_apply_switch_profiles, by the start-up sweep and by analog_key_records_init;
 * lower_bound is then set to 0x0FFF. Offsets in the comments are record offsets (tracker
 * offset + 0x14). */
typedef struct analog_bottom_out_tracker {
    uint8_t  strength_cfg;          /* +0x14 learn_strength the state was built with; a different value in the switch params resets the tracker */
    uint8_t  has_upper;             /* +0x15 upper is valid */
    uint16_t upper;                 /* +0x16 upper estimate: old reference when the reference was lowered, or a promoted candidate */
    uint16_t lower_bound;           /* +0x18 estimate minus a strength-dependent margin; plateaus below it do not move the reference; reset value 0x0FFF */
    uint8_t  strength_eff;          /* +0x1A effective strength after blending, 0..strength_cfg */
    uint8_t  _pad_1b;
    uint16_t cand_base;             /* +0x1C first plateau that exceeded the upper estimate */
    uint16_t cand_max;              /* +0x1E largest such plateau */
    uint8_t  cand_hits;             /* +0x20 plateaus at or above the candidate, promotion at 3 */
    uint8_t  cand_misses;           /* +0x21 plateaus clearly below the candidate; 3 misses remove one hit */
    uint16_t score;                 /* +0x22 0..2000, up on confirming plateaus, down on shallower ones */
    uint16_t confidence;            /* +0x24 min(2000, score + bonus); the scan paths adopt bo_estimate when it is > 149 */
    uint16_t stability;             /* +0x26 0..3000, weight of the reference in the blend */
    uint16_t max_plateau;           /* +0x28 largest accepted plateau; the scan paths limit bottom_level to 229 * max_plateau / 255 */
    uint16_t _pad_2a;
    float    velocity;              /* +0x2C sensor counts per millisecond between the last two accepted samples */
    float    stroke_peak_velocity;  /* +0x30 largest velocity of the current stroke */
    uint8_t  quiet_samples;         /* +0x34 consecutive samples with |velocity| <= 2; 3 start a plateau */
    uint8_t  stroke_active;         /* +0x35 a press stroke is being followed; also cleared by the scan paths on the rest -> pressed edge */
    uint16_t stroke_start_us;       /* +0x36 low 16 bits of hal_time_micros; stroke dropped after 25 ms */
    uint16_t last_sample_us;        /* +0x38 time of the last accepted sample; samples closer than 1000 us are ignored */
    uint16_t last_sample_value;     /* +0x3A 0 = no sample yet */
    uint8_t  plateau_active;        /* +0x3C 1 while a plateau is being collected */
    uint8_t  _pad_3d;
    uint16_t unk_3e;                /* +0x3E never accessed; the plateau block below starts 4-aligned */
    uint16_t plateau_start_us;      /* +0x40 plateau accepted after 15 ms */
    uint16_t plateau_first_value;   /* +0x42 written only */
    uint16_t plateau_max;           /* +0x44 largest sample of the plateau */
    uint16_t plateau_min;           /* +0x46 smallest sample; the value used as "the plateau" */
    float    plateau_mean;          /* +0x48 running mean */
    uint16_t plateau_samples;       /* +0x4C samples added to the plateau */
    uint16_t _pad_4e;
} analog_bottom_out_tracker_t;

_Static_assert(sizeof(analog_bottom_out_tracker_t) == 0x3C, "analog_bottom_out_tracker_t size");
_Static_assert(offsetof(analog_bottom_out_tracker_t, strength_cfg) == 0x00, "bo_tracker.strength_cfg");
_Static_assert(offsetof(analog_bottom_out_tracker_t, has_upper) == 0x01, "bo_tracker.has_upper");
_Static_assert(offsetof(analog_bottom_out_tracker_t, upper) == 0x02, "bo_tracker.upper");
_Static_assert(offsetof(analog_bottom_out_tracker_t, lower_bound) == 0x04, "bo_tracker.lower_bound");
_Static_assert(offsetof(analog_bottom_out_tracker_t, strength_eff) == 0x06, "bo_tracker.strength_eff");
_Static_assert(offsetof(analog_bottom_out_tracker_t, cand_base) == 0x08, "bo_tracker.cand_base");
_Static_assert(offsetof(analog_bottom_out_tracker_t, cand_max) == 0x0A, "bo_tracker.cand_max");
_Static_assert(offsetof(analog_bottom_out_tracker_t, cand_hits) == 0x0C, "bo_tracker.cand_hits");
_Static_assert(offsetof(analog_bottom_out_tracker_t, cand_misses) == 0x0D, "bo_tracker.cand_misses");
_Static_assert(offsetof(analog_bottom_out_tracker_t, score) == 0x0E, "bo_tracker.score");
_Static_assert(offsetof(analog_bottom_out_tracker_t, confidence) == 0x10, "bo_tracker.confidence");
_Static_assert(offsetof(analog_bottom_out_tracker_t, stability) == 0x12, "bo_tracker.stability");
_Static_assert(offsetof(analog_bottom_out_tracker_t, max_plateau) == 0x14, "bo_tracker.max_plateau");
_Static_assert(offsetof(analog_bottom_out_tracker_t, velocity) == 0x18, "bo_tracker.velocity");
_Static_assert(offsetof(analog_bottom_out_tracker_t, stroke_peak_velocity) == 0x1C, "bo_tracker.stroke_peak_velocity");
_Static_assert(offsetof(analog_bottom_out_tracker_t, quiet_samples) == 0x20, "bo_tracker.quiet_samples");
_Static_assert(offsetof(analog_bottom_out_tracker_t, stroke_active) == 0x21, "bo_tracker.stroke_active");
_Static_assert(offsetof(analog_bottom_out_tracker_t, stroke_start_us) == 0x22, "bo_tracker.stroke_start_us");
_Static_assert(offsetof(analog_bottom_out_tracker_t, last_sample_us) == 0x24, "bo_tracker.last_sample_us");
_Static_assert(offsetof(analog_bottom_out_tracker_t, last_sample_value) == 0x26, "bo_tracker.last_sample_value");
_Static_assert(offsetof(analog_bottom_out_tracker_t, plateau_active) == 0x28, "bo_tracker.plateau_active");
_Static_assert(offsetof(analog_bottom_out_tracker_t, plateau_start_us) == 0x2C, "bo_tracker.plateau_start_us");
_Static_assert(offsetof(analog_bottom_out_tracker_t, plateau_first_value) == 0x2E, "bo_tracker.plateau_first_value");
_Static_assert(offsetof(analog_bottom_out_tracker_t, plateau_max) == 0x30, "bo_tracker.plateau_max");
_Static_assert(offsetof(analog_bottom_out_tracker_t, plateau_min) == 0x32, "bo_tracker.plateau_min");
_Static_assert(offsetof(analog_bottom_out_tracker_t, plateau_mean) == 0x34, "bo_tracker.plateau_mean");
_Static_assert(offsetof(analog_bottom_out_tracker_t, plateau_samples) == 0x38, "bo_tracker.plateau_samples");

/* ---- Release-rebound filter ------------------------------------------------------------------- */

/* analog_rebound_filter_t.state (state machine of analog_detect_release_rebound).
 * Travel % = 100 * (clamp(reading, rest, bottom) - rest) / (bottom - rest). */
enum analog_rebound_state {
    ANALOG_REBOUND_IDLE      = 0,  /* travel > 30 % -> PRESSED */
    ANALOG_REBOUND_PRESSED   = 1,  /* reading falls -> RELEASING */
    ANALOG_REBOUND_RELEASING = 2,  /* travel == 0 -> RELEASED; rising with travel > 0 -> IDLE */
    ANALOG_REBOUND_RELEASED  = 3,  /* armed: rising -> REBOUND (events suppressed); 3.5 ms at rest -> IDLE */
    ANALOG_REBOUND_REBOUND   = 4,  /* travel > 10 % -> IDLE (real press); 3.5 ms at rest -> IDLE */
};

/* 8 bytes at analog_key_state_t +0x52, `filter` argument of analog_detect_release_rebound. Only
 * used while g_profile_global_option3_active is set. Offsets in the comments are record offsets. */
typedef struct analog_rebound_filter {
    uint16_t prev_reading;        /* +0x52 reading of the previous call; 0x0FFF at boot and when the feature is switched on */
    uint8_t  rest_timer_running;  /* +0x54 1 while the reading is at or below the rest level */
    uint8_t  _pad_55;
    uint16_t rest_timer_start_us; /* +0x56 low 16 bits of hal_time_micros */
    uint8_t  state;               /* +0x58 enum analog_rebound_state; at rest the filter is only called while it is non-zero */
    uint8_t  _pad_59;
} analog_rebound_filter_t;

_Static_assert(sizeof(analog_rebound_filter_t) == 8, "analog_rebound_filter_t size");
_Static_assert(offsetof(analog_rebound_filter_t, prev_reading) == 0, "rebound.prev_reading");
_Static_assert(offsetof(analog_rebound_filter_t, rest_timer_running) == 2, "rebound.rest_timer_running");
_Static_assert(offsetof(analog_rebound_filter_t, rest_timer_start_us) == 4, "rebound.rest_timer_start_us");
_Static_assert(offsetof(analog_rebound_filter_t, state) == 6, "rebound.state");

/* ---- Per-key record --------------------------------------------------------------------------- */

/* analog_key_state_t.flags (+0x84). The bits are available as masks and as C bit-fields; both
 * views are provided. */
#define ANALOG_KEY_FLAG_SENSOR_STATE  0x01u  /* never set in this firmware: cleared on scan-mode switch, reported as 0xFF / 0x00 by command 20 kind 2 */
#define ANALOG_KEY_FLAG_RT_OVERRIDE   0x02u  /* per-key rapid-trigger setting present */
#define ANALOG_KEY_FLAG_RT_ON         0x04u  /* its value: 1 = rapid trigger on */
#define ANALOG_KEY_FLAG_RT_ENGAGED    0x08u  /* key passed the actuation point and has not left the rapid-trigger zone */
#define ANALOG_KEY_FLAG_PRESSED       0x10u  /* digital state after the actuation logic */
#define ANALOG_KEY_FLAG_CHANGED       0x20u  /* press / release edge pending; consumed by keys_process_scan_pass */
#define ANALOG_KEY_FLAG_DISABLED      0x40u  /* output blocked, toggled by layout_key_action_toggle_key_disable; the key then owns the RGB overlay whose id is its key id */
#define ANALOG_KEY_FLAG_DEFER_OUTPUT  0x80u  /* set on the key whose press resolved a pending mod-tap: output skipped for one pass */

/* analog_key_state_t.flags2 (+0x8D). */
#define ANALOG_KEY_FLAG2_IGNORE_UNTIL_RELEASE 0x01u  /* set by the profile-switch key actions; cleared when travel is 0; the scan paths keep queueing events while set */
#define ANALOG_KEY_FLAG2_LATCHED_LAYER_MASK   0x06u  /* press: active layer, release: locked layer */
#define ANALOG_KEY_FLAG2_LATCHED_LAYER_SHIFT  1u

/* One advanced-key binding of a key, one per keymap layer (+0x85 + 2 * layer). */
typedef struct analog_ak_slot {
    uint8_t index;   /* index into g_keys_advanced_keys entries, 0xFF = none */
    uint8_t active;  /* non-zero while the advanced key still has work to do: it is processed even when its layer is not current and the scan paths keep queueing events for the key at rest */
} analog_ak_slot_t;

_Static_assert(sizeof(analog_ak_slot_t) == 2, "analog_ak_slot_t size");

/* 172 (0xAC) bytes. Element of g_analog_key_states[ANALOG_KEY_COUNT], index = scan position =
 * mux address + BOARD_MUX_POSITIONS * mux index. Reached by scan position, through the scan column
 * table, through scan events / key refs, and through g_layout_keyid_to_scanpos[row * LAYOUT_COLS +
 * col]. Shared between the ADC interrupt and the main loop. */
typedef struct analog_key_state {
    uint16_t actuation_point;      /* +0x00 per-key actuation point in the distance encoding (keys_types.h), 0 = use g_keys_actuation_point; cleared by profile_activate and by analog_key_actuation_reset at start-up */
    uint16_t press_ref_travel;     /* +0x02 reference travel of the press decision: tracked minimum while released / maximum while pressed, or the last rapid-trigger turning point; cleared on keymap load */
    uint16_t noise_band_single;    /* +0x04 noise band measured in single-input scan mode: sweep (uint16)(ceilf((max - min) * 0.5f) * 1.2f), later the idle recalibration */
    uint16_t noise_band_parallel;  /* +0x06 the same for parallel scan mode */
    uint16_t rest_level;           /* +0x08 active rest level of the current scan mode (running maximum during the sweep); loaded by adc_scan_set_mode from +0x80 / +0x82 (+10 when mode > 1); updated by the idle recalibration; key is pressed when value > (uint16_t)(rest_level + 5) */
    uint16_t noise_band;           /* +0x0A active hysteresis width (running minimum during the sweep, analog_key_records_init initialises 0xFFFF); loaded by adc_scan_set_mode from +0x04 / +0x06 (+2 when mode > 1); updated by the idle recalibration */
    uint16_t filtered;             /* +0x0C state of the hysteresis filter; cleared on scan-mode switch */
    uint16_t unk_0e;               /* +0x0E never accessed */
    uint16_t bo_reference;         /* +0x10 lowest accepted bottom-out plateau so far, 0x0FFF = none */
    uint16_t bo_estimate;          /* +0x12 output of the bottom-out tracker, 0x0FFF = none; copied to bottom_level by the scan paths */
    analog_bottom_out_tracker_t bo_tracker; /* +0x14 */
    uint8_t  bottom_is_learned;    /* +0x50 0 = bottom_level is the nominal / stretched estimate, 1 = bottom_level follows bo_estimate */
    uint8_t  _pad_51;
    analog_rebound_filter_t rebound; /* +0x52 */
    uint16_t _pad_5a;
    analog_noise_stats_t noise_stats; /* +0x5C statistics of the idle recalibration (noise_recal variant) */
    uint8_t  recal_active;         /* +0x78 1 = the scan paths feed this key's samples into noise_stats; cleared for all keys when a recalibration pass ends or is aborted */
    uint8_t  _pad_79;
    uint16_t unk_7a;               /* +0x7A never accessed */
    uint16_t value;                /* +0x7C current sensor value: hysteresis-filtered magnitude of the reading, or the unfiltered magnitude while g_run_state_options bit 0 (raw scanning) is set */
    uint16_t bottom_level;         /* +0x7E sensor value at the end of travel: rest + max(50, round(curve_end_q8 * (nominal_bottom - rest) / 0xFFF00)), then maintained by the scan paths */
    uint16_t rest_level_single;    /* +0x80 stored rest level for single-input mode = maximum of the 100 sweep conversions, later the idle recalibration */
    uint16_t rest_level_parallel;  /* +0x82 stored rest level for parallel mode: sweep maximum, + 5 when the parallel noise band is <= 4; later the idle recalibration */
    union {
        uint8_t flags;             /* +0x84 ANALOG_KEY_FLAG_* */
        struct {
            uint8_t sensor_state : 1;  /* bit 0 */
            uint8_t rt_override  : 1;  /* bit 1 */
            uint8_t rt_on        : 1;  /* bit 2 */
            uint8_t rt_engaged   : 1;  /* bit 3 */
            uint8_t pressed      : 1;  /* bit 4 */
            uint8_t changed      : 1;  /* bit 5 */
            uint8_t disabled     : 1;  /* bit 6 */
            uint8_t defer_output : 1;  /* bit 7 */
        };
    };
    analog_ak_slot_t ak_slot[ANALOG_KEY_LAYERS]; /* +0x85 advanced-key binding per layer; all 8 bytes set to 0xFF before the list is re-applied */
    union {
        uint8_t flags2;            /* +0x8D ANALOG_KEY_FLAG2_* */
        struct {
            uint8_t ignore_until_release : 1;  /* bit 0 */
            uint8_t latched_layer        : 2;  /* bits 2:1 */
            uint8_t                      : 5;
        };
    };
    uint8_t  keycode[ANALOG_KEY_LAYERS]; /* +0x8E key code per keymap layer, index into k_layout_keycode_actions, 0xFF = none */
    uint8_t  gamepad_binding;      /* +0x92 direct gamepad control code 120..155 of the key, 0 = none */
    uint8_t  _pad_93;
    RapidTriggerParams *rt_params; /* +0x94 NULL = use g_keys_rapid_trigger_params, else the params of a "prt2" group inside g_keys_rt_groups */
    uint8_t  has_switch_params;    /* +0x98 1 = switch_params is valid, 0 = use g_analog_default_switch_params */
    uint8_t  _pad_99[3];
    analog_switch_params_t switch_params; /* +0x9C per-key run-time switch parameters (analog_apply_switch_profiles) */
} analog_key_state_t;

_Static_assert(sizeof(analog_key_state_t) == 0xAC, "analog_key_state_t size");
_Static_assert(offsetof(analog_key_state_t, actuation_point) == 0x00, "key.actuation_point");
_Static_assert(offsetof(analog_key_state_t, press_ref_travel) == 0x02, "key.press_ref_travel");
_Static_assert(offsetof(analog_key_state_t, noise_band_single) == 0x04, "key.noise_band_single");
_Static_assert(offsetof(analog_key_state_t, noise_band_parallel) == 0x06, "key.noise_band_parallel");
_Static_assert(offsetof(analog_key_state_t, rest_level) == 0x08, "key.rest_level");
_Static_assert(offsetof(analog_key_state_t, noise_band) == 0x0A, "key.noise_band");
_Static_assert(offsetof(analog_key_state_t, filtered) == 0x0C, "key.filtered");
_Static_assert(offsetof(analog_key_state_t, bo_reference) == 0x10, "key.bo_reference");
_Static_assert(offsetof(analog_key_state_t, bo_estimate) == 0x12, "key.bo_estimate");
_Static_assert(offsetof(analog_key_state_t, bo_tracker) == 0x14, "key.bo_tracker");
_Static_assert(offsetof(analog_key_state_t, bottom_is_learned) == 0x50, "key.bottom_is_learned");
_Static_assert(offsetof(analog_key_state_t, rebound) == 0x52, "key.rebound");
_Static_assert(offsetof(analog_key_state_t, noise_stats) == 0x5C, "key.noise_stats");
_Static_assert(offsetof(analog_key_state_t, recal_active) == 0x78, "key.recal_active");
_Static_assert(offsetof(analog_key_state_t, value) == 0x7C, "key.value");
_Static_assert(offsetof(analog_key_state_t, bottom_level) == 0x7E, "key.bottom_level");
_Static_assert(offsetof(analog_key_state_t, rest_level_single) == 0x80, "key.rest_level_single");
_Static_assert(offsetof(analog_key_state_t, rest_level_parallel) == 0x82, "key.rest_level_parallel");
_Static_assert(offsetof(analog_key_state_t, flags) == 0x84, "key.flags");
_Static_assert(offsetof(analog_key_state_t, ak_slot) == 0x85, "key.ak_slot");
_Static_assert(offsetof(analog_key_state_t, flags2) == 0x8D, "key.flags2");
_Static_assert(offsetof(analog_key_state_t, keycode) == 0x8E, "key.keycode");
_Static_assert(offsetof(analog_key_state_t, gamepad_binding) == 0x92, "key.gamepad_binding");
_Static_assert(offsetof(analog_key_state_t, rt_params) == 0x94, "key.rt_params");
_Static_assert(offsetof(analog_key_state_t, has_switch_params) == 0x98, "key.has_switch_params");
_Static_assert(offsetof(analog_key_state_t, switch_params) == 0x9C, "key.switch_params");

/* ---- Idle noise recalibration batches ---------------------------------------------------------- */

/* g_analog_recal_pattern / analog_recal_iter_t.pattern: how a recalibration pass groups the keys
 * into batches. Batches per pattern: k_analog_recal_batch_counts. */
enum analog_recal_pattern {
    ANALOG_RECAL_PATTERN_SINGLE_KEY   = 0,  /* one key per batch, base = scan position */
    ANALOG_RECAL_PATTERN_COLUMN_SPLIT = 1,  /* the keys of one mux address (batch modulo the addresses in use) */
    ANALOG_RECAL_PATTERN_COLUMN       = 2,  /* the keys of mux address batch */
    ANALOG_RECAL_PATTERN_MUX_PAIR     = 3,  /* all positions in use of multiplexers 2 * batch and 2 * batch + 1 */
};

/* 4 bytes on the stack of analog_recal_reset_batch_stats and adc_scan_start_cycle, consumed by
 * analog_recal_batch_next. */
typedef struct analog_recal_iter {
    uint8_t pos;      /* +0 next position inside the batch, advanced by the iterator */
    uint8_t pattern;  /* +1 enum analog_recal_pattern (copy of g_analog_recal_pattern) */
    uint8_t base;     /* +2 pattern 0: scan position; 1, 2: mux address; 3: first multiplexer */
    uint8_t count;    /* +3 patterns 1, 2: end of pos (ADC_SCAN_MUX_COUNT); else 0 */
} analog_recal_iter_t;

_Static_assert(sizeof(analog_recal_iter_t) == 4, "analog_recal_iter_t size");

#endif /* ANALOG_TYPES_H */
