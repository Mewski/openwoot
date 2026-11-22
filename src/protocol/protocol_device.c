/*
 * protocol_device.c - device-level commands of the host protocol: ping / version / acks,
 * restart and update mode, event subscription, USB speed, device settings, factory reset,
 * storage usage and flash self-test, keyboard-wide settings (file "global").
 */
#include <string.h>

#include "tusb.h"
#include "lfs.h"
#include "pb_decode.h"
#include "pb_encode.h"

#include "protocol/protocol.h"
#include "analog/analog.h"
#include "core/clock.h"
#include "drivers/nor_flash.h"
#include "hal/hal_mcu.h"
#include "hal/hal_system.h"
#include "profile/profile.h"
#include "rgb/rgb_frame.h"
#include "storage/storage.h"
#include "tables/protocol_tables.h"
#include "usb/usb.h"

/* Firmware version reported by feature command 1: the stock firmware version this firmware
 * is compatible with, so that the vendor's configuration program accepts the keyboard. */
#define PROTOCOL_VERSION_MAJOR  2u
#define PROTOCOL_VERSION_MINOR  14u
#define PROTOCOL_VERSION_PATCH  1u

/* Encoded size limit of the root file written here (buffer on the stack). */
#define PROTOCOL_GLOBAL_FILE_MAX  56u

#define PROTOCOL_FACTORY_RESET_DELAY_MS  100u

/* Byte 4 of the reply of feature command 19 while the flash of the microcontroller can be
 * read out. */
#define PROTOCOL_READ_UNPROTECTED  0x11u

/* Feature command 0. Reply: one byte, the inverted low argument byte. */
static void protocol_cmd_ping(uint32_t arg, protocol_response_t *response)
{
    response->length = 1u;
    response->payload[0] = (uint8_t)~arg;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_PING, protocol_cmd_ping);

/* Feature command 1. Reply: three bytes major, minor, patch. */
static void protocol_cmd_get_version(uint32_t arg, protocol_response_t *response)
{
    response->length = 3u;
    response->payload[0] = PROTOCOL_VERSION_MAJOR;
    response->payload[1] = PROTOCOL_VERSION_MINOR;
    response->payload[2] = PROTOCOL_VERSION_PATCH;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_VERSION, protocol_cmd_get_version);

/* Feature command 2. Restarts into the bootloader, which stays in update mode; the low
 * argument byte is left for it. Never returns, no response is sent. */
static void protocol_cmd_enter_update_mode(uint32_t arg, protocol_response_t *response)
{
    hal_system_enter_update_mode((uint8_t)arg);
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_RESET_TO_BOOTLOADER, protocol_cmd_enter_update_mode);

/* Feature command 25. Restarts the firmware; a non-zero low argument byte is left for the
 * bootloader. Never returns, no response is sent. */
static void protocol_cmd_restart(uint32_t arg, protocol_response_t *response)
{
    hal_system_restart((uint8_t)arg);
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SOFT_RESET, protocol_cmd_restart);

/* OUT-report command 8, accepted and ignored. */
static void protocol_report_cmd_noop(const uint8_t *report)
{
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_NOOP, protocol_report_cmd_noop);

/* Feature command 24. Reply: status OK with an explicit payload length of 0. */
static void protocol_cmd_ack_empty(uint32_t arg, protocol_response_t *response)
{
    response->length = 0u;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_ACK_EMPTY, protocol_cmd_ack_empty);

/* Feature command 47. Reply: status OK, no payload. */
static void protocol_cmd_ack(uint32_t arg, protocol_response_t *response)
{
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_ACK, protocol_cmd_ack);

/* Feature command 76. arg byte 0 = event tag (0, 1, 2 or 4), arg bit 8 = enable. Reply: one
 * byte, the new subscription state; status 0xFF for any other tag. */
static void protocol_cmd_subscribe_event(uint32_t arg, protocol_response_t *response)
{
    uint8_t tag = (uint8_t)arg;

    if (tag > PROTOCOL_EVENT_PROFILES_CHANGED || tag == PROTOCOL_EVENT_TAG3) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    uint8_t enable = (arg & 0x100u) != 0u;

    g_protocol_event_subscriptions[tag] = enable;
    response->length = 1u;
    response->payload[0] = enable;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SUBSCRIBE_EVENT, protocol_cmd_subscribe_event);

/* Feature command 75. Clears every event subscription. Reply: status OK, no payload. */
static void protocol_cmd_unsubscribe_all_events(uint32_t arg, protocol_response_t *response)
{
    memset(g_protocol_event_subscriptions, 0, sizeof(g_protocol_event_subscriptions));
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_UNSUBSCRIBE_ALL_EVENTS, protocol_cmd_unsubscribe_all_events);

/* Feature command 69. Reply: 1 low speed, 2 full speed, 3 high speed, 0 unknown. */
static void protocol_cmd_get_usb_speed(uint32_t arg, protocol_response_t *response)
{
    response->status = PROTOCOL_STATUS_OK;

    uint32_t speed = tud_speed_get();

    response->length = 1u;
    response->payload[0] = (speed < sizeof(k_protocol_usb_speed_codes)) ? k_protocol_usb_speed_codes[speed] : 0u;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_USB_SPEED, protocol_cmd_get_usb_speed);

/* OUT-report command 7. Stores the device settings; the 7-byte device settings block follows
 * the 3-byte header directly. No response. */
static void protocol_report_cmd_set_device_config(const uint8_t *report)
{
    storage_save_device_settings((const storage_device_settings_t *)(report + offsetof(protocol_report_plain_t, length)));
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_DEVICE_CONFIG, protocol_report_cmd_set_device_config);

/* Feature command 19. Reply: the 7-byte device settings block (all zero if the file system
 * is not mounted), with byte 4 replaced by the read-protection code of the microcontroller,
 * or PROTOCOL_READ_UNPROTECTED if the flash can be read out. */
static void protocol_cmd_get_device_config(uint32_t arg, protocol_response_t *response)
{
    storage_device_settings_t settings;

    memset(&settings, 0, sizeof(settings));
    if (g_storage_mounted != 0u) {
        storage_load_device_settings(&settings);
    }

    /* Read protection of the microcontroller instead of the stored byte 4. */
    uint8_t protection;

    if (!hal_mcu_is_read_protected(&protection)) {
        protection = PROTOCOL_READ_UNPROTECTED;
    }
    settings.mcu_option_byte = protection;

    memcpy(response->payload, &settings, sizeof(settings));
    response->length = sizeof(settings);
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_DEVICE_CONFIG, protocol_cmd_get_device_config);

/* Feature command 43. Erases the whole file system (all profiles and
 * settings) and resets; never returns, no response is sent. */
static void protocol_cmd_factory_reset(uint32_t arg, protocol_response_t *response)
{
    lfs_format(&g_storage_lfs, &g_storage_lfs_config);

    uint32_t deadline = g_clock_wait_ms + PROTOCOL_FACTORY_RESET_DELAY_MS;

    while (deadline > g_clock_wait_ms) {
    }
    hal_system_reset();
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_FACTORY_RESET, protocol_cmd_factory_reset);

/* Feature command 58. Reply: StorageUsage message with the number of file-system blocks in
 * use. g_storage_mounted is not tested. */
static void protocol_cmd_get_storage_usage(uint32_t arg, protocol_response_t *response)
{
    StorageUsage usage;

    /* Blocks in use, or the negative littlefs error of the traversal. */
    usage.blocks_in_use = lfs_fs_size(&g_storage_lfs);

    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);

    if (pb_encode(&stream, &StorageUsage_msg, &usage)) {
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_STORAGE_USAGE, protocol_cmd_get_storage_usage);

/* Feature command 56. Checks the id of the SPI flash and increments the
 * counter file "boot_count". Reply: 1 = every step succeeded. g_storage_mounted is not
 * tested. */
static void protocol_cmd_flash_selftest(uint32_t arg, protocol_response_t *response)
{
    uint8_t buffer[STORAGE_LFS_CACHE_SIZE]; /* flash ids first, then the file cache */
    uint8_t passed = 0u;

    buffer[0] = 0u;
    buffer[1] = 0u;
    nor_flash_read_id(buffer);

    uint8_t manufacturer = buffer[0];
    uint8_t device_id = buffer[1];

    /* The unique id is read but not used. */
    nor_flash_read_unique_id(buffer);

    if ((manufacturer == NOR_FLASH_MFR_WINBOND || manufacturer == NOR_FLASH_MFR_GIGADEVICE)
        && device_id == NOR_FLASH_DEVICE_ID_64MBIT) {
        lfs_file_t file;
        struct lfs_file_config file_cfg = { .buffer = buffer, .attrs = NULL, .attr_count = 0u };
        uint32_t boot_count = 0u;

        /* KNOWN ISSUE: when a step after the open fails the file is not closed and stays
         * linked in the list of open files of g_storage_lfs although it lives on this stack.
         * Matches stock firmware 2.14.1. */
        if (lfs_file_opencfg(&g_storage_lfs, &file, "boot_count", STORAGE_OPEN_COUNTER, &file_cfg) >= 0
            && lfs_file_read(&g_storage_lfs, &file, &boot_count, sizeof(boot_count)) >= 0) {
            boot_count++;
            if (lfs_file_seek(&g_storage_lfs, &file, 0, LFS_SEEK_SET) >= 0
                && lfs_file_write(&g_storage_lfs, &file, &boot_count, sizeof(boot_count)) >= 0
                && lfs_file_close(&g_storage_lfs, &file) >= 0) {
                passed = 1u;
            }
        }
    }

    response->payload[0] = passed;
    response->length = 1u;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_FLASH_SELFTEST, protocol_cmd_flash_selftest);

/* Feature command 51. Reply: the GlobalSettings message read from the file "global" (not
 * the RAM copy g_profile_global_settings); empty payload if the file does not exist. */
static void protocol_cmd_get_global_settings(uint32_t arg, protocol_response_t *response)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);
    GlobalSettings settings = GlobalSettings_init_default;

    if (g_storage_mounted == 0u) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    int result = storage_read_pb_file("global", &GlobalSettings_msg, &settings);

    if (result == STORAGE_OK) {
        if (!pb_encode(&stream, &GlobalSettings_msg, &settings)) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else if (result == LFS_ERR_NOENT) {
        response->length = 0u;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_GLOBAL_SETTINGS, protocol_cmd_get_global_settings);

/* OUT-report command 23. Applies the keyboard-wide settings and, with the
 * persist flag, writes them to the file "global". No response. */
static void protocol_cmd_set_global_settings(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;
    GlobalSettings settings = GlobalSettings_init_default;
    pb_istream_t stream = pb_istream_from_buffer(report->payload, report->length);

    if (!pb_decode(&stream, &GlobalSettings_msg, &settings)) {
        return;
    }

    rgb_reset_idle_timer();

    if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) != 0u && g_storage_mounted != 0u) {
        uint8_t encoded[PROTOCOL_GLOBAL_FILE_MAX];
        pb_ostream_t file_stream = pb_ostream_from_buffer(encoded, sizeof(encoded));

        /* The result of the encoder is not checked. */
        pb_encode(&file_stream, &GlobalSettings_msg, &settings);
        storage_write_file("global", encoded, (uint16_t)file_stream.bytes_written);
    }

    g_profile_global_settings = settings;

    /* Release-rebound filter: when it is switched on every key starts with a fresh filter. */
    uint8_t filter_active = settings.has_release_rebound_filter_enabled;

    if (filter_active != 0u) {
        filter_active = g_profile_global_settings.release_rebound_filter_enabled;
        if (filter_active != 0u && g_profile_global_option3_active == 0u) {
            for (uint32_t key = 0u; key < ANALOG_KEY_COUNT; key++) {
                analog_rebound_filter_t *filter = &g_analog_key_states[key].rebound;

                filter->prev_reading = ANALOG_VALUE_NONE;
                filter->rest_timer_running = 0u;
                filter->rest_timer_start_us = 0u;
                filter->state = ANALOG_REBOUND_IDLE;
            }
        }
    }
    g_profile_global_option3_active = filter_active;

    rgb_update_lock_indicators();
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_GLOBAL_SETTINGS, protocol_cmd_set_global_settings);
