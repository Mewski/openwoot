/*
 * storage: RAM globals (extern declarations). Definitions and descriptions:
 * storage_data.c.
 */
#ifndef STORAGE_DATA_H
#define STORAGE_DATA_H

#include "storage_types.h"

/* Initialised */
extern struct lfs_config g_storage_lfs_config;

/* Zero-initialised */
extern uint8_t  g_storage_mounted;
extern lfs_t    g_storage_lfs;
extern uint8_t  g_storage_revision_bump_disabled;
extern uint32_t g_storage_lfs_lookahead_buffer[STORAGE_LFS_LOOKAHEAD_SIZE / 4];
extern uint8_t  g_storage_lfs_prog_buffer[STORAGE_LFS_CACHE_SIZE];
extern uint8_t  g_storage_lfs_read_buffer[STORAGE_LFS_CACHE_SIZE];

#endif /* STORAGE_DATA_H */
