/*
 * device_info.c, variant mcu_flash_page_readonly of the strategy slot device_info: the device
 * information record is kept in a flash page of the microcontroller at BOARD_DEVICE_INFO_ADDR,
 * outside the application image, and is read with hal_mcu_flash_read. This firmware never
 * writes it: the variant contains no code that erases or programs flash and calls none, so the
 * factory record of the unit cannot be lost through it.
 *
 * Besides the record format of device_info.h the reader accepts an older raw record (a page
 * without marker that is not blank), as the stock firmware 2.14.1 does.
 *
 * Depends on: nanopb, hal_mcu (flash read), storage/device_info/device_info.h, board.h.
 */
#include <stdint.h>

#include <pb_decode.h>

#include "hal/hal_mcu.h"
#include "storage/storage.h"

#define DEVICE_INFO_MAGIC_ADDR  (BOARD_DEVICE_INFO_ADDR + STORAGE_DEVICE_INFO_MAGIC_OFFSET)

_Static_assert(BOARD_DEVICE_INFO_ADDR >= BOARD_FLASH_END, "the record lies outside the application image");

/*
 * Older raw record: the fields of SerialNumber without the three optional ones, then an
 * encoded DeviceInfoAux behind a length byte. It carries no physical layout and no lock mode.
 */
typedef struct __attribute__((packed)) {
    uint16_t f1;         /* 0x00: SerialNumber tag 1 */
    uint8_t  f2;         /* 0x02: tag 2 */
    uint8_t  f3;         /* 0x03: tag 3 */
    uint16_t f4;         /* 0x04: tag 4 */
    uint16_t f5;         /* 0x06: tag 5 */
    uint16_t f6;         /* 0x08: tag 6 */
    uint8_t  stage;      /* 0x0A: tag 7 */
    uint8_t  unused;     /* 0x0B: not read */
    uint8_t  aux_length; /* 0x0C: 1..254: an encoded DeviceInfoAux follows; 0 or 255: none */
    uint8_t  aux[44];    /* 0x0D */
} raw_device_record_t;

_Static_assert(sizeof(raw_device_record_t) == 57, "raw_device_record_t size");

/* Fills the serial number and the auxiliary record from the older raw record. Returns
 * STORAGE_OK, or STORAGE_ERR_DECODE if the auxiliary record does not decode. */
static int device_info_read_raw_record(const raw_device_record_t *raw, DeviceInfo *info)
{
    info->serial_number.f1 = raw->f1;
    info->serial_number.f2 = raw->f2;
    info->serial_number.f3 = raw->f3;
    info->serial_number.f4 = raw->f4;
    info->serial_number.f5 = raw->f5;
    info->serial_number.f6 = raw->f6;
    info->serial_number.stage = raw->stage;
    info->serial_number.has_edition = false;
    info->serial_number.has_f10 = false;
    info->serial_number.has_f11 = false;

    if (raw->aux_length == 0u || raw->aux_length > STORAGE_DEVICE_INFO_LEN8_MAX) {
        info->has_f2 = false;
        return STORAGE_OK;
    }

    /* KNOWN ISSUE: the length is checked against 254, not against the 44 bytes of the record,
     * so a longer value decodes the bytes that follow the record in the page. Matches stock
     * firmware 2.14.1, which decodes a copy on its stack and reads beyond the copy instead. */
    pb_istream_t stream = pb_istream_from_buffer(raw->aux, raw->aux_length);
    if (!pb_decode(&stream, &DeviceInfoAux_msg, &info->f2)) {
        return STORAGE_ERR_DECODE;
    }
    info->has_f2 = true;
    return STORAGE_OK;
}

/*
 * The marker selects the length prefix; a page without marker is blank or holds the older raw
 * record.
 */
int storage_read_device_info(DeviceInfo *info)
{
    uint16_t magic;
    uint8_t record[STORAGE_DEVICE_INFO_AREA_SIZE];

    hal_mcu_flash_read(DEVICE_INFO_MAGIC_ADDR, (uint8_t *)&magic, sizeof(magic));
    hal_mcu_flash_read(BOARD_DEVICE_INFO_ADDR, record, sizeof(record));
    if (magic == STORAGE_DEVICE_INFO_MAGIC_LEN8 || magic == STORAGE_DEVICE_INFO_MAGIC_LEN16) {
        return storage_device_info_decode(magic, record, info);
    }
    if (storage_device_info_is_blank(record)) {
        return STORAGE_ERR_BLANK;
    }
    return device_info_read_raw_record((const raw_device_record_t *)record, info);
}

/*
 * Stores nothing. The host commands that change the device information (OUT reports 12
 * serial number, 28 auxiliary record, 39 lock mode) therefore change nothing that survives a
 * reset; a lock mode set by the host holds in RAM until then. The stock firmware 2.14.1
 * rewrites the page.
 */
int storage_write_device_info(const DeviceInfo *info)
{
    (void)info;
    return STORAGE_ERR_READ_ONLY;
}
