/*
 * storage.h - public functions of the storage module: the littlefs volume on the settings flash
 * chip, whole files, the files of a profile directory, the device settings, the serial number
 * text, and through the two strategy slot headers the device information record and the
 * physical layout of the unit.
 *
 * Depends on: proto/settings.pb.h, littlefs, the strategy slot headers
 * storage/device_info/device_info.h and storage/layout_variant/layout_variant.h.
 */
#ifndef STORAGE_STORAGE_H_
#define STORAGE_STORAGE_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "proto/settings.pb.h"
#include "storage/storage_types.h"
#include "storage/storage_data.h"
#include "storage/device_info/device_info.h"
#include "storage/layout_variant/layout_variant.h"

/* Counts the consecutive existing profile directories of a namespace. */
int storage_count_profile_dirs(uint32_t ns, uint8_t *count);
/* Removes a profile directory with all of its files; built-in profile 0 is protected. */
int storage_delete_profile_dir(uint16_t profile_id);
/* Formats the serial number text; returns the snprintf result. */
int storage_format_serial_string(const SerialNumber *serial, uint32_t size, char *out);
/* Formats the path of file `name` of a profile ("<index>/<name>" or "lp/<index>/<name>"). */
int storage_format_profile_path(char *out, uint16_t profile_id, const char *name);
/* Replaces the content of a file; returns the byte count or a negative littlefs error. */
int storage_write_file(const char *path, const void *data, uint32_t size);
/* Reads up to size bytes of a file; returns the byte count or a negative littlefs error. */
int storage_read_file(const char *path, void *buf, uint32_t size);
/* Reads the key colours of one layer (file "r<layer>", STORAGE_LAYER_COLORS_SIZE bytes). */
int storage_read_layer_colors(uint16_t profile_id, uint32_t layer, void *out);
/* Reads the key map of one layer (file "ml<layer>", STORAGE_KEYMAP_LAYER_SIZE bytes). */
int storage_read_keymap_layer(uint16_t profile_id, void *out, uint32_t layer);
/* Returns 1 if the file exists and is not empty. */
int storage_file_exists(const char *path);
/* Counts the consecutive non-empty files "al0".."al15" of a profile. */
int storage_count_al_files(uint16_t profile_id, uint8_t *count);
/* Copies the serial number text into the flash page of the microcontroller that the
 * bootloader reads (BOARD_MCU_SERIAL_PAGE_ADDR). */
void storage_mirror_serial_to_mcu_flash(void);
/* littlefs block-device callbacks: read and erase. */
int storage_bd_read(const struct lfs_config *cfg, uint32_t block, uint32_t off, void *buffer, uint32_t size);
int storage_bd_erase(const struct lfs_config *cfg, uint32_t block);
/* littlefs block-device callback: sync (waits until the flash is idle). */
int storage_bd_sync(const struct lfs_config *cfg);
/* littlefs block-device callback: program. */
int storage_bd_prog(const struct lfs_config *cfg, uint32_t block, uint32_t off, const void *buffer, uint32_t size);
/* Reads a file and decodes it as a nanopb message. */
int storage_read_pb_file(const char *path, const pb_msgdesc_t *msgdesc, void *msg);
/* Fills the packed device-settings block from file "ds" and the stored physical layout. */
void storage_load_device_settings(storage_device_settings_t *settings);
/* Stores the packed device-settings block: file "ds", then the physical layout where the
 * board keeps it. */
void storage_save_device_settings(const storage_device_settings_t *settings);
/* Increments the revision counter file "pst" of a profile. */
void storage_bump_profile_revision(uint16_t profile_id);
/* Writes the key map of one layer (file "ml<layer>"). */
int storage_write_keymap_layer(uint16_t profile_id, const void *keymap, uint32_t layer);
/* Writes the key colours of one layer (file "r<layer>"). */
int storage_write_layer_colors(uint16_t profile_id, uint32_t layer, const void *colors);
/* Writes the base colours (file "clr", STORAGE_BASE_COLORS_SIZE bytes). */
int storage_write_base_colors(uint16_t profile_id, const void *colors);
/* Encodes a nanopb message and writes it as file `name` of a profile. */
int storage_write_profile_pb_file(uint16_t profile_id, const char *name, const pb_msgdesc_t *msgdesc, uint32_t max_size, const void *msg);
/* Writes the advanced keys (file "ak2") and removes the legacy file "dks". */
int storage_save_advanced_keys(uint16_t profile_id, const void *msg);

#endif /* STORAGE_STORAGE_H_ */
