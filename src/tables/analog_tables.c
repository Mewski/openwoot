/* Constant tables of module `analog` (definitions): properties of the hall-switch types 0..48, and
 * the batch counts of the idle recalibration, which follow from the board's scan geometry. */
#include "analog_tables.h"

#include "board.h"

/* Total travel per hall-switch type id 0..48 in 0.05 mm units (80 = 4.0 mm) */
const uint8_t k_analog_switch_travel[49] = {
    80, 80, 66, 68, 68, 68, 70, 68, 68, 66, 68, 64, 68, 66, 70, 70,
    64, 68, 66, 68, 68, 70, 69, 64, 70, 68, 68, 70, 68, 68, 66, 64,
    66, 70, 80, 76, 68, 70, 64, 70, 64, 80, 68, 68, 68, 70, 70, 68,
    68,
};

/* Value 0..16 per switch type id, sent as optional field 5 of the switch-type list
 * (protocol_cmd_get_switch_types); the firmware does not use it otherwise */
const uint8_t k_analog_switch_group_id[49] = {
     0,  0,  0,  7,  6,  6,  6,  6,  6,  2,  3,  5, 11, 11, 14, 16,
     4,  3, 10,  7,  1,  6,  6, 13,  8,  9, 11, 15,  7,  8,  4, 12,
    14,  2,  4, 10,  7,  0,  0,  0,  0,  7,  9,  6,  6, 10, 11, 11,
    11,
};

/* Bottom-out tracking strength per switch type id (default of switch parameter byte +0x0C) */
const uint8_t k_analog_switch_bottom_out_strength[49] = {
    255,  70,  44,  57,  59,  59,  59,  59,  59,  38,  63,  51,  45,  63, 114,  79,
     63,  63,  42,  96,  44,  79,  49,  51,  20,  33,  96,  57,  96,  96,  38,  51,
     44,  44, 255,  95,  42,  45,  51,  22,  45, 108,  20,  31,  43,  39,  22,  22,
     30,
};

/* Switch type id -> curve number 0..39 in k_analog_travel_curves */
const uint8_t k_analog_switch_curve_index[49] = {
     0,  1,  2,  3,  4,  5,  5,  4,  5,  6,  7,  8,  9, 10, 11, 12,
    13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 16, 16,  6, 20,
     2, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 35, 36, 37, 38,
    39,
};

/* Per switch type id, value 700..1450 (switch parameter +0x0E): nominal ADC level of the fully pressed key */
const uint16_t k_analog_switch_bottom_adc[49] = {
    1200, 1250,  900, 1000,  950,  950,  950,  950,  950,  930,  900,  700, 1000, 1000, 1000, 1250,
     720,  930, 1100,  930,  950,  850,  950,  950, 1450, 1150,  970, 1250,  930,  930,  930,  950,
     900, 1200, 1000, 1050, 1200, 1050, 1070, 1300,  800, 1120, 1300, 1170, 1170, 1230, 1200, 1200,
    1150,
};

/* Number of key batches per recalibration pattern 0..3 (enum analog_recal_pattern;
 * adc_scan_start_cycle): one per scan position; one per multiplexer address in use (twice); one per
 * pair of multiplexers. */
const uint8_t k_analog_recal_batch_counts[4] = {
    BOARD_SCAN_POSITIONS,
    BOARD_MUX_ADDRESSES_USED,
    BOARD_MUX_ADDRESSES_USED,
    (BOARD_MUX_COUNT + 1) / 2,
};
