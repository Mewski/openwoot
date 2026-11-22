/*
 * device_info.c, variant nor_flash_sector of the strategy slot device_info: the device
 * information record is kept in one sector of the settings flash chip behind the littlefs
 * volume, at offset BOARD_DEVICE_INFO_ADDR. Reading copies the record into RAM; writing erases
 * the sector and programs the record, then the marker.
 *
 * Depends on: drivers/nor_flash.h, nanopb, storage/device_info/device_info.h, board.h.
 */
#include <string.h>

#include <pb_encode.h>

#include "drivers/nor_flash.h"
#include "storage/storage.h"

#define DEVICE_INFO_MAGIC_ADDR  (BOARD_DEVICE_INFO_ADDR + STORAGE_DEVICE_INFO_MAGIC_OFFSET)

_Static_assert(BOARD_DEVICE_INFO_ADDR % STORAGE_LFS_BLOCK_SIZE == 0u,
               "the record starts at an erase sector of the chip");
_Static_assert(BOARD_DEVICE_INFO_ADDR >= BOARD_STORAGE_LFS_BLOCK_COUNT * STORAGE_LFS_BLOCK_SIZE,
               "the record lies behind the littlefs volume");

/*
 * The marker selects the length prefix; a sector without marker is taken as the 1-byte
 * format unless it is blank.
 */
int storage_read_device_info(DeviceInfo *info)
{
    uint16_t magic = 0xFFFFu;
    uint8_t record[STORAGE_DEVICE_INFO_AREA_SIZE];

    nor_flash_wait_ready();
    nor_flash_read(DEVICE_INFO_MAGIC_ADDR, (uint8_t *)&magic, sizeof(magic));
    if (magic != STORAGE_DEVICE_INFO_MAGIC_LEN8 && magic != STORAGE_DEVICE_INFO_MAGIC_LEN16) {
        uint8_t probe[STORAGE_DEVICE_INFO_BLANK_PROBE];

        nor_flash_wait_ready();
        nor_flash_read(BOARD_DEVICE_INFO_ADDR, probe, sizeof(probe));
        if (storage_device_info_is_blank(probe)) {
            return STORAGE_ERR_BLANK;
        }
        magic = STORAGE_DEVICE_INFO_MAGIC_LEN8;
    }

    memset(record, 0, sizeof(record));
    nor_flash_wait_ready();
    nor_flash_read(BOARD_DEVICE_INFO_ADDR, record, sizeof(record));
    return storage_device_info_decode(magic, record, info);
}

/*
 * Erases the sector, then programs the record and the marker last. No verification and not
 * power-fail safe. The return value is incidental: 0 if the encode failed or after the
 * sector was written, 1 if the record was too long to store.
 */
int storage_write_device_info(const DeviceInfo *info)
{
    uint16_t magic;
    uint32_t total;
    uint8_t record[STORAGE_DEVICE_INFO_AREA_SIZE];

    memset(record, 0, sizeof(record));
    /* Encoded behind a 2-byte length prefix; moved down by one byte for the short format. */
    pb_ostream_t stream = pb_ostream_from_buffer(&record[2], STORAGE_DEVICE_INFO_LEN16_MAX);
    if (!pb_encode(&stream, &DeviceInfo_msg, info)) {
        return 0;
    }

    uint16_t length = (uint16_t)stream.bytes_written;
    if (length <= STORAGE_DEVICE_INFO_LEN8_MAX) {
        memmove(&record[1], &record[2], length);
        record[0] = (uint8_t)length;
        magic = STORAGE_DEVICE_INFO_MAGIC_LEN8;
        total = (uint16_t)(length + 1u);
    } else {
        total = (uint16_t)(length + 2u);
        memcpy(record, &length, sizeof(length));
        magic = STORAGE_DEVICE_INFO_MAGIC_LEN16;
        if (total > STORAGE_DEVICE_INFO_AREA_SIZE) {
            return 1;
        }
    }

    nor_flash_wait_ready();
    nor_flash_erase_sector(BOARD_DEVICE_INFO_ADDR);
    nor_flash_wait_ready();
    nor_flash_program(BOARD_DEVICE_INFO_ADDR, record, total);
    nor_flash_wait_ready();
    nor_flash_program(DEVICE_INFO_MAGIC_ADDR, (const uint8_t *)&magic, sizeof(magic));
    nor_flash_wait_ready();
    return 0;
}
