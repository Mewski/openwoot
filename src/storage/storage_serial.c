/*
 * storage_serial.c - the serial number text of the keyboard: formatted from the factory
 * device information, and mirrored into a flash page of the microcontroller, where the
 * bootloader reads it as [length byte][text] without the settings flash.
 *
 * Depends on: core/startup.h, hal_mcu (flash read and page write), the device information of
 * the storage module, board.h (BOARD_MCU_SERIAL_PAGE_ADDR).
 */
#include <stdio.h>
#include <string.h>

#include "board.h"
#include "core/startup.h"
#include "hal/hal_mcu.h"
#include "storage/storage.h"
#include "tables/storage_tables.h"

#define STORAGE_SERIAL_PART_MAX         5u      /* "T%02u" / "S%02u" / "%02u" + NUL */
#define STORAGE_SERIAL_STAGE_DEFAULT    'H'

/*
 * Formats the serial number text. Only the low byte of the 16-bit fields is
 * printed. Returns the snprintf result.
 */
int storage_format_serial_string(const SerialNumber *serial, uint32_t size, char *out)
{
    char stage_char;
    char pcb_part[STORAGE_SERIAL_PART_MAX] = { 0 };
    char minor_part[STORAGE_SERIAL_PART_MAX] = { 0 };
    char edition_part[STORAGE_SERIAL_PART_MAX] = { 0 };

    uint8_t stage_index = (uint8_t)(serial->stage - 1u);
    if (stage_index < STORAGE_SERIAL_STAGE_COUNT) {
        stage_char = k_storage_serial_stage_chars[stage_index];
    } else {
        stage_char = STORAGE_SERIAL_STAGE_DEFAULT;
    }

    if (serial->has_f10) {
        snprintf(pcb_part, sizeof(pcb_part), "T%02u", serial->f10);
    }
    if (serial->has_f11) {
        snprintf(minor_part, sizeof(minor_part), "%02u", serial->f11);
    }
    if (serial->has_edition) {
        snprintf(edition_part, sizeof(edition_part), "S%02u", serial->edition);
    }

    return snprintf(out, size, "A%02uB%02u%02uW%02u%s%01u%s%s%c%05lu",
                    (uint8_t)serial->f1, serial->f2, serial->f3, (uint8_t)serial->f4,
                    pcb_part, (uint8_t)serial->f5, minor_part, edition_part, stage_char,
                    (unsigned long)serial->f6);
}

/*
 * Copies the serial number text into the flash page of the microcontroller at
 * BOARD_MCU_SERIAL_PAGE_ADDR as [length byte][text]. Nothing happens on a microcontroller
 * that does not write its own flash.
 */
void storage_mirror_serial_to_mcu_flash(void)
{
    SerialNumber serial;
    DeviceInfo info;
    /* Record: length byte, then the text (at most 50 bytes including the NUL). */
    uint8_t record[56];

    memset(&serial, 0, sizeof(serial));
    memset(&info, 0, sizeof(info));
    if (storage_read_device_info(&info) != STORAGE_OK) {
        return;
    }
    serial = info.serial_number;

    int length = storage_format_serial_string(&serial, STORAGE_MCU_SERIAL_TEXT_MAX, (char *)&record[1]);
    if (length < 0) {
        return;
    }
    if (length >= 255) {
        length = 255;
    }
    record[0] = (uint8_t)length;

    /* KNOWN ISSUE: `length` is the untruncated snprintf result. A serial text longer than
     * 49 characters would make the write program stack bytes behind the record. The
     * format yields 26 to 31 characters for field values below 100. Matches stock
     * firmware 2.14.1. */
    (void)hal_mcu_flash_write_page(BOARD_MCU_SERIAL_PAGE_ADDR, record, 1u + (uint32_t)length);
}

/* A blank serial page (first byte 0xFF) is filled from the device information. */
static void device_serial_mirror_init(void)
{
    uint8_t length;

    hal_mcu_flash_read(BOARD_MCU_SERIAL_PAGE_ADDR, &length, sizeof(length));
    if (length == 0xFF) {
        storage_mirror_serial_to_mcu_flash();
    }
}
STARTUP_STEP(STARTUP_SERIAL_MIRROR, device_serial_mirror_init);
