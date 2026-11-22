/*
 * storage: littlefs block device on the settings flash chip
 * (callbacks of g_storage_lfs_config; block = 4 KiB sector).
 *
 * None of the callbacks can report an error: the flash commands have no
 * status feedback beyond the busy bit.
 */
#include "drivers/nor_flash.h"
#include "storage/storage.h"

/* Reads size bytes at byte offset off of a block. */
int storage_bd_read(const struct lfs_config *cfg, uint32_t block, uint32_t off, void *buffer, uint32_t size)
{
    nor_flash_wait_ready();
    /* The size is truncated to 16 bits. Matches stock firmware 2.14.1. */
    nor_flash_read(off + cfg->block_size * block, (uint8_t *)buffer, (uint16_t)size);
    return LFS_ERR_OK;
}

/* Erases one 4 KiB sector. Does not wait for the erase to finish; the next flash access does. */
int storage_bd_erase(const struct lfs_config *cfg, uint32_t block)
{
    nor_flash_wait_ready();
    nor_flash_erase_sector(cfg->block_size * block);
    return LFS_ERR_OK;
}

/* Waits until the flash has finished the last program or erase cycle. */
int storage_bd_sync(const struct lfs_config *cfg)
{
    nor_flash_wait_ready();
    return LFS_ERR_OK;
}

/* Programs size bytes at byte offset off of a block; the block must be erased. */
int storage_bd_prog(const struct lfs_config *cfg, uint32_t block, uint32_t off, const void *buffer, uint32_t size)
{
    nor_flash_wait_ready();
    nor_flash_program(off + cfg->block_size * block, (const uint8_t *)buffer, size);
    return LFS_ERR_OK;
}
