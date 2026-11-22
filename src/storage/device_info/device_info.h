/*
 * device_info.h - interface of the strategy slot device_info: where the factory device
 * information record of the unit (serial number, auxiliary record, physical layout, lock mode)
 * is kept, and the record format that every variant shares.
 *
 * The board selects one variant with STRATEGY_device_info in its board.mk; the variant's
 * device_info.c implements storage_read_device_info and storage_write_device_info.
 * BOARD_DEVICE_INFO_ADDR is the address of the record in the medium the variant names. The
 * format helpers are implemented once, in storage/storage_device_info.c.
 *
 * Record format (the same in every medium):
 *   BOARD_DEVICE_INFO_ADDR           length (1 byte with marker 0x1337, 2 bytes little endian
 *                                    with marker 0x1338), then the encoded DeviceInfo message
 *   BOARD_DEVICE_INFO_ADDR + 0xFFC   16-bit marker, written last
 *
 * Depends on: the settings schema (proto/settings.pb.h).
 */
#ifndef STORAGE_DEVICE_INFO_DEVICE_INFO_H
#define STORAGE_DEVICE_INFO_DEVICE_INFO_H

#include <stdbool.h>
#include <stdint.h>

#include "proto/settings.pb.h"

#define STORAGE_DEVICE_INFO_MAGIC_LEN8      0x1337u /* 1-byte length prefix */
#define STORAGE_DEVICE_INFO_MAGIC_LEN16     0x1338u /* 2-byte length prefix */
#define STORAGE_DEVICE_INFO_MAGIC_OFFSET    0xFFCu  /* marker behind the record area */
#define STORAGE_DEVICE_INFO_AREA_SIZE       0xFFCu  /* length prefix and payload */
#define STORAGE_DEVICE_INFO_LEN8_MAX        254u    /* longest payload of the 1-byte length format */
#define STORAGE_DEVICE_INFO_LEN16_MAX       0xFFAu  /* longest payload of the 2-byte length format */
#define STORAGE_DEVICE_INFO_BLANK_PROBE     30u     /* bytes tested for 0xFF when no marker is found */

/* ---- Implemented by the selected variant ------------------------------------------------- */

/* Reads and decodes the record into `info`, which the caller has zeroed. Returns STORAGE_OK,
 * STORAGE_ERR_BLANK if nothing is stored, or STORAGE_ERR_DECODE. */
int storage_read_device_info(DeviceInfo *info);

/* Stores `info` as the new record, if the variant can. No caller reads the result: the host
 * commands that change the record send no reply. */
int storage_write_device_info(const DeviceInfo *info);

/* ---- Record format, shared by the variants ----------------------------------------------- */

/* True if the first STORAGE_DEVICE_INFO_BLANK_PROBE bytes of `start` are erased (0xFF). */
bool storage_device_info_is_blank(const uint8_t *start);

/* Decodes the record at `record` (length prefix and payload) in the format that `magic` names.
 * Returns STORAGE_OK, or STORAGE_ERR_DECODE for a bad length or a message that does not
 * decode. */
int storage_device_info_decode(uint16_t magic, const uint8_t *record, DeviceInfo *info);

#endif /* STORAGE_DEVICE_INFO_DEVICE_INFO_H */
