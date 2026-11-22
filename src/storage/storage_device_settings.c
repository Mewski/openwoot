/*
 * storage_device_settings.c - the device settings: littlefs root file "ds" (message
 * DeviceSettings) and the physical layout, exchanged with the start-up code and the host as
 * the packed 7-byte block storage_device_settings_t. Where the physical layout is kept is the
 * strategy layout_variant.
 *
 * Depends on: nanopb, the file helpers (storage_file.c), storage/layout_variant/layout_variant.h.
 */
#include <pb_encode.h>

#include "storage/storage.h"

#define STORAGE_DS_ENCODED_MAX  8u /* encode buffer of file "ds" */

/*
 * Fills the packed device-settings block from file "ds", then the physical layout. Bits not
 * listed in storage_types.h keep the value passed in. The caller checks g_storage_mounted.
 */
void storage_load_device_settings(storage_device_settings_t *settings)
{
    DeviceSettings ds = DeviceSettings_init_default;

    if (storage_read_pb_file("ds", &DeviceSettings_msg, &ds) == STORAGE_OK) {
        uint8_t option_bits = (uint8_t)(((uint8_t)ds.usb_full_speed << 3) | (((uint8_t)ds.nkro_enabled ^ 1u) << 4));
        settings->flags = (uint8_t)((settings->flags & ~(STORAGE_DS_FLAG_USB_FULL_SPEED | STORAGE_DS_FLAG_6KRO))
                                    | (option_bits & (STORAGE_DS_FLAG_USB_FULL_SPEED | STORAGE_DS_FLAG_6KRO)));
        settings->usb_flags = (uint8_t)((settings->usb_flags & ~STORAGE_DS_USB_MODE_MASK)
                                        | ((ds.usb_gamepad_mode & 3u) << STORAGE_DS_USB_MODE_SHIFT));
        uint8_t poll_interval = ds.has_gamepad_poll_interval
                                    ? (uint8_t)(ds.gamepad_poll_interval & 3u)
                                    : (uint8_t)GamepadPollInterval_GAMEPAD_POLL_INTERVAL_1MS;
        settings->usb_flags = (uint8_t)((settings->usb_flags & ~STORAGE_DS_USB_POLL_MASK)
                                        | (poll_interval << STORAGE_DS_USB_POLL_SHIFT));
    } else {
        settings->flags = 0u;
        settings->usb_flags = (uint8_t)((settings->usb_flags & ~(STORAGE_DS_USB_MODE_MASK | STORAGE_DS_USB_POLL_MASK))
                                        | STORAGE_DS_USB_DEFAULT);
    }

    settings->layout_variant = storage_layout_variant_load();
}

/*
 * Stores the packed device-settings block: file "ds", then the physical layout (only if the
 * file was written).
 */
void storage_save_device_settings(const storage_device_settings_t *settings)
{
    DeviceSettings ds;
    uint8_t encoded[STORAGE_DS_ENCODED_MAX];

    if (g_storage_mounted == 0u) {
        return;
    }

    uint8_t poll_interval = (settings->usb_flags & STORAGE_DS_USB_POLL_MASK) >> STORAGE_DS_USB_POLL_SHIFT;
    if (poll_interval == 3u) {
        poll_interval = GamepadPollInterval_GAMEPAD_POLL_INTERVAL_1MS;
    }
    ds.nkro_enabled = (settings->flags & STORAGE_DS_FLAG_6KRO) == 0u;
    ds.usb_gamepad_mode = (UsbGamepadMode)((settings->usb_flags & STORAGE_DS_USB_MODE_MASK) >> STORAGE_DS_USB_MODE_SHIFT);
    ds.usb_full_speed = (settings->flags & STORAGE_DS_FLAG_USB_FULL_SPEED) != 0u;
    ds.has_gamepad_poll_interval = true;
    ds.gamepad_poll_interval = (GamepadPollInterval)poll_interval;

    pb_ostream_t stream = pb_ostream_from_buffer(encoded, sizeof(encoded));
    pb_encode(&stream, &DeviceSettings_msg, &ds); /* result ignored, matches stock firmware 2.14.1 */
    int written = storage_write_file("ds", encoded, (uint16_t)stream.bytes_written);
    if (written > 0 || (int16_t)written == 1) {
        storage_layout_variant_store(settings->layout_variant);
    }
}
