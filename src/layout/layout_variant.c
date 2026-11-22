/*
 * layout_variant.c - selection of the physical layout variant: points the run-time matrix lookups
 * at the tables of the board's variant entry (k_layout_variants).
 */
#include "core/startup.h"
#include "layout/layout.h"
#include "rgb/rgb_data.h"
#include "tables/layout_tables.h"

/*
 * Points the matrix lookup tables at the set of the physical layout variant: key -> LED, row-5
 * columns that mirror the space bar, scan position <-> key id. A variant number at or above
 * BOARD_LAYOUT_VARIANT_COUNT (settings_load_and_apply never stores one) gets the maps of variant 0
 * and no space-bar fill columns, as in stock firmware 2.14.1.
 */
void layout_select_variant(void)
{
    uint32_t number = g_layout_physical_variant;
    const layout_variant_t *variant = &k_layout_variants[number < BOARD_LAYOUT_VARIANT_COUNT ? number : 0u];

    g_rgb_key_to_led = variant->key_to_led;
    g_layout_scanpos_to_keyid = variant->scanpos_to_keyid;
    g_layout_keyid_to_scanpos = variant->keyid_to_scanpos;
    if (number < BOARD_LAYOUT_VARIANT_COUNT) {
        g_layout_spacebar_fill_cols = variant->spacebar_fill_cols;
        g_layout_spacebar_fill_col_count = variant->spacebar_fill_col_count;
    } else {
        g_layout_spacebar_fill_cols = NULL;
        g_layout_spacebar_fill_col_count = 0u;
    }
}
STARTUP_STEP(STARTUP_LAYOUT_VARIANT, layout_select_variant);
