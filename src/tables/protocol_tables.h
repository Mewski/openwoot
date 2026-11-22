/* Constant tables of module `protocol` (declarations); each table is described at its definition in
 * protocol_tables.c. Depends on board.h for the dimensions of the scan order table. */
#ifndef TABLES_PROTOCOL_TABLES_H
#define TABLES_PROTOCOL_TABLES_H

#include <stdint.h>

#include "board.h"

extern const uint8_t k_protocol_usb_speed_codes[3];
extern const uint8_t k_protocol_sensor_scan_order[BOARD_MUX_COUNT][BOARD_MUX_ADDRESSES_USED];

#endif /* TABLES_PROTOCOL_TABLES_H */
