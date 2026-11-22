/*
 * storage_types.h - types and constants of the storage module: geometry of the littlefs
 * volume, file helpers, the decoded device information record, device settings.
 *
 * Depends on: board.h (key matrix, BOARD_STORAGE_LFS_BLOCK_COUNT), littlefs, the settings
 * schema (proto/settings.pb.h).
 */
#ifndef STORAGE_TYPES_H
#define STORAGE_TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "board.h"
#include "lfs.h"
#include "proto/settings.pb.h"

/* ------------------------------------------------------------------------- */
/* littlefs port                                                             */
/* ------------------------------------------------------------------------- */

/* Geometry stored in g_storage_lfs_config. The number of blocks is a board number,
 * BOARD_STORAGE_LFS_BLOCK_COUNT: the volume starts at offset 0 of the chip and ends where the
 * board keeps something else on it. */
#define STORAGE_LFS_READ_SIZE           1u
#define STORAGE_LFS_PROG_SIZE           1u
#define STORAGE_LFS_BLOCK_SIZE          0x1000u /* one erase sector of the chip (nor_flash_erase_sector) */
#define STORAGE_LFS_BLOCK_CYCLES        500
#define STORAGE_LFS_CACHE_SIZE          128u    /* also the size of the per-file buffer on the callers' stacks */
#define STORAGE_LFS_LOOKAHEAD_SIZE      64u     /* bytes = 512 blocks per window */

/* The layouts this module depends on (littlefs v2.4, no LFS_THREADSAFE, no LFS_MIGRATE). */
_Static_assert(sizeof(lfs_t) == 0x78, "lfs_t size");
_Static_assert(offsetof(lfs_t, rcache) == 0x00, "lfs_t.rcache");
_Static_assert(offsetof(lfs_t, pcache) == 0x10, "lfs_t.pcache");
_Static_assert(offsetof(lfs_t, root) == 0x20, "lfs_t.root");
_Static_assert(offsetof(lfs_t, mlist) == 0x28, "lfs_t.mlist");
_Static_assert(offsetof(lfs_t, seed) == 0x2C, "lfs_t.seed");
_Static_assert(offsetof(lfs_t, gstate) == 0x30, "lfs_t.gstate");
_Static_assert(offsetof(lfs_t, gdisk) == 0x3C, "lfs_t.gdisk");
_Static_assert(offsetof(lfs_t, gdelta) == 0x48, "lfs_t.gdelta");
_Static_assert(offsetof(lfs_t, free) == 0x54, "lfs_t.free");
_Static_assert(offsetof(lfs_t, cfg) == 0x68, "lfs_t.cfg");
_Static_assert(offsetof(lfs_t, name_max) == 0x6C, "lfs_t.name_max");
_Static_assert(offsetof(lfs_t, file_max) == 0x70, "lfs_t.file_max");
_Static_assert(offsetof(lfs_t, attr_max) == 0x74, "lfs_t.attr_max");

_Static_assert(sizeof(struct lfs_config) == 0x4C, "lfs_config size");
_Static_assert(offsetof(struct lfs_config, context) == 0x00, "lfs_config.context");
_Static_assert(offsetof(struct lfs_config, read) == 0x04, "lfs_config.read");
_Static_assert(offsetof(struct lfs_config, prog) == 0x08, "lfs_config.prog");
_Static_assert(offsetof(struct lfs_config, erase) == 0x0C, "lfs_config.erase");
_Static_assert(offsetof(struct lfs_config, sync) == 0x10, "lfs_config.sync");
_Static_assert(offsetof(struct lfs_config, read_size) == 0x14, "lfs_config.read_size");
_Static_assert(offsetof(struct lfs_config, prog_size) == 0x18, "lfs_config.prog_size");
_Static_assert(offsetof(struct lfs_config, block_size) == 0x1C, "lfs_config.block_size"); /* storage_bd_* */
_Static_assert(offsetof(struct lfs_config, block_count) == 0x20, "lfs_config.block_count");
_Static_assert(offsetof(struct lfs_config, block_cycles) == 0x24, "lfs_config.block_cycles");
_Static_assert(offsetof(struct lfs_config, cache_size) == 0x28, "lfs_config.cache_size");
_Static_assert(offsetof(struct lfs_config, lookahead_size) == 0x2C, "lfs_config.lookahead_size");
_Static_assert(offsetof(struct lfs_config, read_buffer) == 0x30, "lfs_config.read_buffer");
_Static_assert(offsetof(struct lfs_config, prog_buffer) == 0x34, "lfs_config.prog_buffer");
_Static_assert(offsetof(struct lfs_config, lookahead_buffer) == 0x38, "lfs_config.lookahead_buffer");
_Static_assert(offsetof(struct lfs_config, name_max) == 0x3C, "lfs_config.name_max");
_Static_assert(offsetof(struct lfs_config, file_max) == 0x40, "lfs_config.file_max");
_Static_assert(offsetof(struct lfs_config, attr_max) == 0x44, "lfs_config.attr_max");
_Static_assert(offsetof(struct lfs_config, metadata_max) == 0x48, "lfs_config.metadata_max");

_Static_assert(sizeof(lfs_file_t) == 0x54, "lfs_file_t size"); /* stack objects of the file helpers */
_Static_assert(sizeof(lfs_dir_t) == 0x34, "lfs_dir_t size");   /* stack object of storage_count_profile_dirs */

/* Open flags used by the file helpers. */
#define STORAGE_OPEN_READ       LFS_O_RDONLY                                /* 0x001: storage_read_file */
#define STORAGE_OPEN_COUNTER    (LFS_O_RDWR | LFS_O_CREAT)                  /* 0x103: "pst", "boot_count" */
#define STORAGE_OPEN_REWRITE    (LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC)  /* 0x502: storage_write_file */

_Static_assert(STORAGE_OPEN_READ == 0x001, "open flags of the read helpers");
_Static_assert(STORAGE_OPEN_COUNTER == 0x103, "open flags of the counter files");
_Static_assert(STORAGE_OPEN_REWRITE == 0x502, "open flags of storage_write_file");

/* Result convention of the storage helpers. Other
 * negative values are littlefs errors truncated to int16 (LFS_ERR_NOENT = -2 is
 * tested by the callers); storage_write_file / storage_read_file return byte counts. */
enum storage_result {
    STORAGE_OK              = 1,
    STORAGE_ERR_NOT_MOUNTED = -1, /* g_storage_mounted == 0 */
    STORAGE_ERR_BLANK       = -2, /* no device information stored; same value as LFS_ERR_NOENT */
    STORAGE_ERR_DECODE      = -3, /* pb_decode failed or bad length */
    STORAGE_ERR_PROTECTED   = -4, /* storage_delete_profile_dir refuses built-in profile 0 */
    STORAGE_ERR_READ_ONLY   = -5, /* the device information is kept where this firmware never writes */
};

/* Path buffers: storage_format_profile_path copies the file name through a
 * 16-byte scratch buffer (name used as the sprintf format). */
#define STORAGE_PROFILE_FILE_NAME_MAX   16u

/* Raw (non-protobuf) per-profile files. */
#define STORAGE_BASE_COLORS_SIZE    ((unsigned)(BOARD_KEY_ROWS * BOARD_KEYMAP_COLS) * 2u) /* "<dir>/clr":   RGB565 by [row][keymap column] */
#define STORAGE_LAYER_COLORS_SIZE   ((unsigned)(BOARD_KEY_ROWS * BOARD_KEY_COLS) * 2u)    /* "<dir>/r<k>":  RGB565 by [row][matrix column] */
#define STORAGE_KEYMAP_LAYER_SIZE   ((unsigned)(BOARD_KEY_ROWS * BOARD_KEYMAP_COLS))      /* "<dir>/ml<k>": key codes by [row][keymap column] */
#define STORAGE_AL_FILE_MAX         16u  /* "lp/<n>/al0".."al15" (storage_count_al_files) */

/* ------------------------------------------------------------------------- */
/* Device information, decoded                                               */
/* ------------------------------------------------------------------------- */

/*
 * Where and in which format the record is stored: storage/device_info/device_info.h.
 *
 * Decoded record: DeviceInfo, 48 bytes. Offsets used by the code:
 *   0x00 SerialNumber (20 bytes); layout below
 *   0x14 bool          has_ of tag 2
 *   0x16 DeviceInfoAux (22 bytes); only stored and returned
 *   0x2C bool          has_ of tag 3
 *   0x2D uint8_t       tag 3: physical layout variant -> device settings byte 5 (strategy
 *                      layout_variant/device_info)
 *   0x2E bool          has_ of tag 4
 *   0x2F uint8_t       tag 4: lock mode -> g_profile_lock_mode (device_info_load_lock_mode)
 *
 * SerialNumber, as printed by storage_format_serial_string with the format
 * "A%02uB%02u%02uW%02u%s%01u%s%s%c%05lu" (the letters follow the public Wooting
 * serial number format):
 *   0x00 uint16_t tag 1   "A%02u" (low byte); 0xFFFF = serial not programmed
 *   0x02 uint8_t  tag 2   "B%02u"
 *   0x03 uint8_t  tag 3   "%02u"
 *   0x04 uint16_t tag 4   "W%02u" (low byte)
 *   0x06 uint16_t tag 5   "%01u"  (low byte)
 *   0x08 uint32_t tag 6   "%05lu"
 *   0x0C uint8_t  tag 7   stage 1..4 -> k_storage_serial_stage_chars "PTEX", anything else 'H'
 *   0x0D bool / 0x0E uint8_t  tag 9   "S%02u" if present; edition code (feature edition)
 *   0x0F bool / 0x10 uint8_t  tag 10  "T%02u" if present
 *   0x11 bool / 0x12 uint8_t  tag 11  "%02u"  if present
 */
_Static_assert(sizeof(DeviceInfo) == 48, "DeviceInfo size");
_Static_assert(sizeof(SerialNumber) == 20, "SerialNumber size");
_Static_assert(sizeof(DeviceInfoAux) == 22, "DeviceInfoAux size");

#define STORAGE_SERIAL_STRING_MAX   35u /* g_usb_serial_string_buf size passed by usb_serial_string_init */
#define STORAGE_SERIAL_STAGE_COUNT  4u  /* entries of k_storage_serial_stage_chars */

/* Copy of the serial text in a flash page of the microcontroller, at
 * BOARD_MCU_SERIAL_PAGE_ADDR, for the bootloader (storage_mirror_serial_to_mcu_flash):
 * [length byte][text]. */
#define STORAGE_MCU_SERIAL_TEXT_MAX     50u     /* snprintf size */

/* ------------------------------------------------------------------------- */
/* Device settings                                                           */
/* ------------------------------------------------------------------------- */

/*
 * File "ds": message DeviceSettings, 5 bytes (struct defaults {1, 0, 0, has = 0, 1}):
 *   0x00 bool     tag 1  0 = 6KRO keyboard descriptor        <-> settings.flags bit 4 (inverted)
 *   0x01 uint8_t  tag 2  USB mode 0..3                       <-> settings.usb_flags bits 2:1
 *   0x02 bool     tag 3  force USB full speed                <-> settings.flags bit 3
 *   0x03 bool     has_ of tag 4
 *   0x04 uint8_t  tag 4  gamepad polling rate selector       <-> settings.usb_flags bits 4:3 (1 if absent)
 */
_Static_assert(sizeof(DeviceSettings) == 5, "size of message DeviceSettings");

/*
 * Packed device-settings block: the 7 payload bytes of feature command 19
 * (protocol_cmd_get_device_config) and of OUT-report command 7 (report + 3).
 * Filled by storage_load_device_settings, stored by storage_save_device_settings; the
 * device-settings start-up steps keep one for the whole start-up.
 */
typedef struct __attribute__((packed)) {
    uint8_t flags;            /* 0x0: STORAGE_DS_FLAG_* */
    uint8_t unk_1[3];         /* 0x1: never read or written by the firmware (callers pass zeros) */
    uint8_t mcu_option_byte;  /* 0x4: only in the reply of command 19: read protection code of the microcontroller */
    uint8_t layout_variant;   /* 0x5: physical layout variant (strategy layout_variant), 0 if none is stored */
    uint8_t usb_flags;        /* 0x6: STORAGE_DS_USB_* */
} storage_device_settings_t;

_Static_assert(sizeof(storage_device_settings_t) == 7, "storage_device_settings_t size");
_Static_assert(offsetof(storage_device_settings_t, flags) == 0, "storage_device_settings_t.flags");
_Static_assert(offsetof(storage_device_settings_t, mcu_option_byte) == 4, "storage_device_settings_t.mcu_option_byte");
_Static_assert(offsetof(storage_device_settings_t, layout_variant) == 5, "storage_device_settings_t.layout_variant");
_Static_assert(offsetof(storage_device_settings_t, usb_flags) == 6, "storage_device_settings_t.usb_flags");

/* storage_device_settings_t.flags */
#define STORAGE_DS_FLAG_USB_FULL_SPEED  0x08u /* bit 3: "ds" tag 3 -> g_run_state_options bit 1 (settings_load_and_apply) */
#define STORAGE_DS_FLAG_6KRO            0x10u /* bit 4: !("ds" tag 1) -> g_run_state_flags bits 1 and 2 (settings_load_and_apply) */
#define STORAGE_DS_FLAG_LEGACY          0x80u /* bit 7: never set by the loader; settings_load_and_apply clears it, sets USB mode 1 and saves */

/* storage_device_settings_t.usb_flags */
#define STORAGE_DS_USB_MODE_SHIFT       1u
#define STORAGE_DS_USB_MODE_MASK        0x06u /* bits 2:1: USB mode -> g_usb_link_flags bits 1:0; 3 is rewritten to 2 by settings_apply_usb_mode */
#define STORAGE_DS_USB_POLL_SHIFT       3u
#define STORAGE_DS_USB_POLL_MASK        0x18u /* bits 4:3: gamepad polling rate -> g_usb_gamepad_poll_rate bits 1:0; 3 is stored as 1 */
#define STORAGE_DS_USB_DEFAULT          0x0Cu /* bits 4:1 written when "ds" cannot be read: mode 2, polling rate 1 */

#endif /* STORAGE_TYPES_H */
