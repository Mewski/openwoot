/*
 * storage_device_info.c - the format of the device information record, shared by every variant
 * of the strategy slot device_info (storage/device_info/device_info.h): the blank test and the
 * decoding of a record that is already in memory. Where the record lives, and whether it can
 * be written, is the variant's business.
 *
 * Depends on: nanopb, the settings schema.
 */
#include <string.h>

#include <pb_decode.h>

#include "storage/storage.h"

bool storage_device_info_is_blank(const uint8_t *start)
{
    for (uint32_t i = 0u; i < STORAGE_DEVICE_INFO_BLANK_PROBE; i++) {
        if (start[i] != 0xFFu) {
            return false;
        }
    }
    return true;
}

int storage_device_info_decode(uint16_t magic, const uint8_t *record, DeviceInfo *info)
{
    const uint8_t *payload;
    size_t length;

    if (magic == STORAGE_DEVICE_INFO_MAGIC_LEN16) {
        uint16_t length16;
        memcpy(&length16, record, sizeof(length16));
        if (length16 == 0u || length16 > STORAGE_DEVICE_INFO_LEN16_MAX) {
            return STORAGE_ERR_DECODE;
        }
        length = length16;
        payload = &record[2];
    } else {
        uint8_t length8 = record[0];
        if (length8 == 0u || length8 > STORAGE_DEVICE_INFO_LEN8_MAX) {
            return STORAGE_ERR_DECODE;
        }
        length = length8;
        payload = &record[1];
    }

    pb_istream_t stream = pb_istream_from_buffer(payload, length);
    if (!pb_decode(&stream, &DeviceInfo_msg, info)) {
        return STORAGE_ERR_DECODE;
    }
    return STORAGE_OK;
}
