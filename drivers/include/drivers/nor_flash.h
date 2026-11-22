/*
 * nor_flash.h - what the storage needs from the settings flash chip: reads, page programs,
 * sector erases, the busy wait and the two identification queries with the ids they answer.
 *
 * Implemented by exactly one driver of class nor_flash (drivers/<driver>/driver.mk) that the
 * board lists. Addresses are byte offsets into the chip. Program and erase return as soon as
 * the chip has accepted the command; nothing but nor_flash_program waits for a previous cycle,
 * so every other access is preceded by nor_flash_wait_ready.
 *
 * Depends on: nothing.
 */
#ifndef DRIVERS_NOR_FLASH_H
#define DRIVERS_NOR_FLASH_H

#include <stdint.h>

/* Sets up the bus to the chip; the chip is left deselected. */
void nor_flash_init(void);

/* Waits until the chip has finished its last program or erase cycle; the USB device task
 * keeps running meanwhile. No timeout. */
void nor_flash_wait_ready(void);

/* Reads `len` bytes at `addr`. */
void nor_flash_read(uint32_t addr, uint8_t *buf, uint32_t len);

/* Programs `len` bytes at `addr` into erased flash, split at the page boundaries of the chip;
 * waits for the chip before every page. */
void nor_flash_program(uint32_t addr, const uint8_t *data, uint32_t len);

/* Erases the 4 KiB sector that starts at `addr`. */
void nor_flash_erase_sector(uint32_t addr);

/* Reads the manufacturer and device id: id[0] manufacturer, id[1] device. */
void nor_flash_read_id(uint8_t id[2]);

/* Answers of nor_flash_read_id: manufacturer ids (JEDEC) and the device id of a 64 Mbit chip. */
#define NOR_FLASH_MFR_WINBOND       0xEFu
#define NOR_FLASH_MFR_GIGADEVICE    0xC8u
#define NOR_FLASH_DEVICE_ID_64MBIT  0x16u

/* Reads the 64-bit unique id of the chip. */
void nor_flash_read_unique_id(uint8_t id[8]);

#endif /* DRIVERS_NOR_FLASH_H */
