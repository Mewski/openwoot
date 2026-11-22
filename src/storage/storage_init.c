/*
 * storage_init.c - the start-up steps of the storage module: the settings flash chip, the
 * littlefs volume on it, and the lock mode of the factory device information.
 *
 * Depends on: core/startup.h, the NOR flash driver class, hal_system (format request of the
 * bootloader), the profile module (lock mode).
 */
#include <string.h>

#include "core/startup.h"
#include "drivers/nor_flash.h"
#include "hal/hal_system.h"
#include "profile/profile.h"
#include "storage/storage.h"

/* The bus of the settings flash chip comes first: every later step reads the volume. */
STARTUP_STEP(STARTUP_NOR_FLASH, nor_flash_init);

/*
 * Mounts the littlefs volume. A request to format it that the bootloader left across the
 * reset (nothing in this firmware makes one) is taken and the volume formatted first. A
 * volume that does not mount is formatted and mounted again. g_storage_mounted is set
 * whatever the second mount returned.
 */
static void storage_init(void)
{
    if (hal_system_take_format_request()) {
        lfs_format(&g_storage_lfs, &g_storage_lfs_config);
    }

    if (lfs_mount(&g_storage_lfs, &g_storage_lfs_config) != 0) {
        lfs_format(&g_storage_lfs, &g_storage_lfs_config);
        lfs_mount(&g_storage_lfs, &g_storage_lfs_config);
    }
    /* KNOWN ISSUE: set although the second lfs_mount may have failed; on a flash chip
     * that cannot be formatted the file helpers then run on an unmounted lfs_t.
     * Matches stock firmware 2.14.1. */
    g_storage_mounted = 1;
}
STARTUP_STEP(STARTUP_STORAGE, storage_init);

/* Reads the factory device information record and caches its lock mode field. */
static void device_info_load_lock_mode(void)
{
    DeviceInfo info;

    memset(&info, 0, sizeof(info));
    if (storage_read_device_info(&info) == STORAGE_OK && info.has_lock_mode) {
        g_profile_lock_mode = info.lock_mode;
    }
}
STARTUP_STEP(STARTUP_LOCK_MODE, device_info_load_lock_mode);
