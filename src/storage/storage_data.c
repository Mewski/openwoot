/*
 * storage_data.c - RAM globals of the storage module. g_storage_lfs_config has an initial
 * value (the block-device callbacks of storage_bd.c and the board's volume size), the other
 * objects start as zero.
 */
#include "storage/storage.h"

/* ---- initialised ---------------------------------------------------------- */

/* littlefs configuration, passed to lfs_format / lfs_mount; everything else goes
 * through g_storage_lfs.cfg. Never written at run time, but not const. */
struct lfs_config g_storage_lfs_config = {
    .context          = NULL,
    .read             = storage_bd_read,
    .prog             = storage_bd_prog,
    .erase            = storage_bd_erase,
    .sync             = storage_bd_sync,
    .read_size        = STORAGE_LFS_READ_SIZE,
    .prog_size        = STORAGE_LFS_PROG_SIZE,
    .block_size       = STORAGE_LFS_BLOCK_SIZE,
    .block_count      = BOARD_STORAGE_LFS_BLOCK_COUNT,
    .block_cycles     = STORAGE_LFS_BLOCK_CYCLES,
    .cache_size       = STORAGE_LFS_CACHE_SIZE,
    .lookahead_size   = STORAGE_LFS_LOOKAHEAD_SIZE,
    .read_buffer      = g_storage_lfs_read_buffer,
    .prog_buffer      = g_storage_lfs_prog_buffer,
    .lookahead_buffer = g_storage_lfs_lookahead_buffer,
    .name_max         = 0, /* littlefs default LFS_NAME_MAX (255) */
    .file_max         = 0, /* littlefs default LFS_FILE_MAX (0x7FFFFFFF) */
    .attr_max         = 0, /* littlefs default LFS_ATTR_MAX (1022) */
    .metadata_max     = 0, /* littlefs default: block_size */
};

/* ---- zero-initialised ----------------------------------------------------- */

/* Set to 1 by storage_init after the mount sequence (lfs_mount, on error lfs_format +
 * lfs_mount), whatever the result of the second mount: it means "mount was
 * attempted". The file helpers return STORAGE_ERR_NOT_MOUNTED while it is 0. */
uint8_t g_storage_mounted;

/* The one littlefs instance. Application code touches it only through the lfs_*
 * functions (storage_count_profile_dirs and storage_delete_profile_dir also
 * open directories on it). */
lfs_t g_storage_lfs;

/* While 1, storage_bump_profile_revision does nothing. Set by storage_create_first_boot_profiles
 * only while the first-boot default files are created. */
uint8_t g_storage_revision_bump_disabled;

/* Static littlefs buffers named in g_storage_lfs_config. */
uint32_t g_storage_lfs_lookahead_buffer[STORAGE_LFS_LOOKAHEAD_SIZE / 4];
uint8_t  g_storage_lfs_prog_buffer[STORAGE_LFS_CACHE_SIZE];
uint8_t  g_storage_lfs_read_buffer[STORAGE_LFS_CACHE_SIZE];
