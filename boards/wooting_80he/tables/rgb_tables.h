/*
 * rgb_tables.h - lighting data of the board: key -> LED maps per layout variant, the LED number
 * -> frame position table and the frame page layout. Each table is described at its definition
 * in rgb_tables.c.
 *
 * Depends on board.h for the dimensions and on rgb/rgb_frame_layout.h for the element types.
 */
#ifndef TABLES_RGB_TABLES_H
#define TABLES_RGB_TABLES_H

#include <stdint.h>

#include "board.h"
#include "rgb/rgb_frame_layout.h"

extern const uint8_t k_rgb_key_to_led_iso[BOARD_KEY_ROWS][BOARD_KEY_COLS];
extern const uint8_t k_rgb_key_to_led_jis[BOARD_KEY_ROWS][BOARD_KEY_COLS];
extern const uint8_t k_rgb_key_to_led_ansi[BOARD_KEY_ROWS][BOARD_KEY_COLS];
extern const rgb_led_channel_t k_rgb_led_channels[BOARD_LED_LIMIT];
extern const rgb_frame_page_layout_t k_rgb_frame_pages[BOARD_RGB_FRAME_PAGE_COUNT];

#endif /* TABLES_RGB_TABLES_H */
