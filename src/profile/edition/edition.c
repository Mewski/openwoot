/*
 * edition.c - recognises a unit of the board's special edition by the edition code of its
 * factory serial number, and answers what the edition changes: USB product string, number and
 * content of the factory profiles, default switch type.
 *
 * Depends on: the storage module (device info), the edition constants of board.h, the edition's
 * factory profiles in the board's factory defaults.
 */
#include <string.h>

#include "profile/edition/edition.h"
#include "storage/storage.h"

/* Edition codes of the serial number that mark a unit of the special edition. */
static const uint8_t k_profile_edition_codes[] = { BOARD_EDITION_SERIAL_CODES };

/* True once the device info was read; s_special is the answer from then on. */
static bool s_known;
static bool s_special;

/* True once profile_edition_default_switch_type decided; s_switch_type is the answer from then
 * on. */
static bool s_switch_type_known;
static uint8_t s_switch_type;

bool profile_edition_is_special(void)
{
    DeviceInfo info;

    if (s_known) {
        return s_special;
    }
    memset(&info, 0, sizeof(info));
    if (storage_read_device_info(&info) != STORAGE_OK) {
        return s_special;
    }

    s_special = false;
    if (info.serial_number.has_edition) {
        for (size_t i = 0; i < sizeof(k_profile_edition_codes); i++) {
            if (info.serial_number.edition == k_profile_edition_codes[i]) {
                s_special = true;
            }
        }
    }
    s_known = true;
    return s_special;
}

const char *profile_edition_product_string(void)
{
    return profile_edition_is_special() ? BOARD_EDITION_USB_PRODUCT : NULL;
}

uint8_t profile_edition_factory_profile_count(void)
{
    return profile_edition_is_special() ? BOARD_EDITION_FACTORY_PROFILE_COUNT : PROFILE_FACTORY_COUNT;
}

const profile_default_set_t *profile_edition_default_set(const profile_default_set_t *regular)
{
    return profile_edition_is_special() ? &k_profile_edition_default_set : regular;
}

uint8_t profile_edition_default_switch_type(void)
{
    if (!s_switch_type_known) {
        s_switch_type = profile_edition_is_special() ? BOARD_EDITION_DEFAULT_SWITCH_TYPE
                                                     : PROFILE_EDITION_REGULAR_SWITCH_TYPE;
        s_switch_type_known = true;
    }
    return s_switch_type;
}
