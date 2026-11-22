/* Constant tables of module `analog` (declarations); each table is described at its definition in
 * analog_tables.c and analog_travel_curves.c. */
#ifndef TABLES_ANALOG_TABLES_H
#define TABLES_ANALOG_TABLES_H

#include <stdint.h>

extern const uint8_t k_analog_switch_travel[49];
extern const uint8_t k_analog_switch_group_id[49];
extern const uint8_t k_analog_switch_bottom_out_strength[49];
extern const uint8_t k_analog_switch_curve_index[49];
extern const uint16_t k_analog_switch_bottom_adc[49];
extern const uint8_t k_analog_recal_batch_counts[4];
extern const uint16_t k_analog_travel_curves[40][4096];

#endif /* TABLES_ANALOG_TABLES_H */
