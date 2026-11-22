/*
 * layout_variant.h - interface of the strategy slot layout_variant: where the physical layout
 * of the unit (an index of k_layout_variants) is kept. It is byte 5 of the device-settings
 * block, which storage_load_device_settings and storage_save_device_settings exchange with the
 * start-up code and the host.
 *
 * The board selects one variant with STRATEGY_layout_variant in its board.mk; the variant's
 * layout_variant.c implements both functions.
 *
 * Depends on: nothing.
 */
#ifndef STORAGE_LAYOUT_VARIANT_LAYOUT_VARIANT_H
#define STORAGE_LAYOUT_VARIANT_LAYOUT_VARIANT_H

#include <stdint.h>

/* The stored physical layout; 0 if none is stored. */
uint8_t storage_layout_variant_load(void);

/* Stores `variant` as the physical layout, if the variant can; otherwise does nothing. Called
 * after the file "ds" was written. */
void storage_layout_variant_store(uint8_t variant);

#endif /* STORAGE_LAYOUT_VARIANT_LAYOUT_VARIANT_H */
