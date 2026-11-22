/*
 * layout_variant.c, variant mcu_option of the strategy slot layout_variant: the physical
 * layout is bit 0 of the user option byte of the microcontroller (index 0 or 1 of
 * k_layout_variants). This firmware never writes the option byte, so storing does nothing and
 * the device settings file "ds" carries no layout. Matches stock firmware 2.14.1.
 *
 * Depends on: hal/hal_mcu.h.
 */
#include "hal/hal_mcu.h"
#include "storage/layout_variant/layout_variant.h"

#define LAYOUT_VARIANT_OPTION_MASK  0x01u /* bit of the user option byte */

uint8_t storage_layout_variant_load(void)
{
    return (uint8_t)(hal_mcu_read_user_option() & LAYOUT_VARIANT_OPTION_MASK);
}

void storage_layout_variant_store(uint8_t variant)
{
    (void)variant;
}
