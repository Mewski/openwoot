/*
 * profile_first_boot.c - the start-up step that writes the factory default profiles when
 * the settings volume holds no profile yet.
 *
 * Depends on: core/startup.h, the storage module (directories, revision counters), the
 * factory defaults of the board (profile_create_factory_defaults), feature `edition` (number
 * of factory profiles).
 */
#include <stdio.h>

#include "core/startup.h"
#include "profile/profile.h"
#include "profile/edition/edition.h"
#include "storage/storage.h"

/*
 * First boot (no built-in profile directory exists): creates one directory per factory
 * profile ("0", "1", ...) and writes the factory default profiles into them.
 * Profile revision counters are not touched while the defaults are written.
 */
static void storage_create_first_boot_profiles(void)
{
    uint8_t count = 0;
    char path[12];
    uint32_t index;

    /* The result is not tested: a failed directory scan leaves count at 0
     * and is taken for a first boot. */
    storage_count_profile_dirs(PROFILE_NS_BUILTIN, &count);
    if (count != 0) {
        return;
    }

    g_storage_revision_bump_disabled = 1;
    for (index = 0; (uint8_t)index < profile_edition_factory_profile_count(); index++) {
        sprintf(path, "%d", (int)index);
        lfs_mkdir(&g_storage_lfs, path);
    }
    profile_create_factory_defaults();
    g_storage_revision_bump_disabled = 0;
}
STARTUP_STEP(STARTUP_FIRST_BOOT_PROFILES, storage_create_first_boot_profiles);
