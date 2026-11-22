/*
 * usb_init.c - the start-up steps of the usb module: the speed option, the descriptors
 * finished from the device settings, and the start of the TinyUSB device stack.
 *
 * Depends on: hal_usb.h, TinyUSB, the storage module (device info), feature `edition` (product
 * string of the unit).
 */
#include <string.h>

#include "tusb.h"

#include "core/run_state.h"
#include "core/startup.h"
#include "hal/hal_usb.h"
#include "profile/edition/edition.h"
#include "storage/storage.h"
#include "tables/usb_tables.h"
#include "usb/usb.h"

STARTUP_STEP(STARTUP_USB_PREPARE, hal_usb_prepare);

/* Device setting "force USB full speed", applied before the stack starts. */
void usb_speed_option_init(void)
{
    if ((g_run_state_options & RUN_STATE_OPTION_USB_FULL_SPEED) != 0) {
        hal_usb_limit_to_full_speed();
    }
}
STARTUP_STEP(STARTUP_USB_SPEED, usb_speed_option_init);

/*
 * Serial number string descriptor: formatted from the factory device info; an
 * unreadable record gives the text of an all-zero serial number.
 */
void usb_serial_string_init(void)
{
    SerialNumber serial;
    DeviceInfo info;

    memset(&serial, 0, sizeof(serial));
    memset(&info, 0, sizeof(info));
    if (storage_read_device_info(&info) == STORAGE_OK) {
        serial = info.serial_number;
    }

    memset(g_usb_serial_string_buf, 0, sizeof(g_usb_serial_string_buf));
    storage_format_serial_string(&serial, sizeof(g_usb_serial_string_buf), g_usb_serial_string_buf);
    g_usb_string_table.serial = g_usb_serial_string_buf;
}
STARTUP_STEP(STARTUP_USB_SERIAL_STRING, usb_serial_string_init);

/*
 * Product string of the unit's edition, if it has one (BOARD_USB_PRODUCT otherwise); bcdDevice
 * 0x0250 (NKRO) or 0x0251 (6KRO); idProduct USB_PID_BASE + USB mode; keyboard report descriptor
 * 6KRO or NKRO.
 */
void usb_descriptors_init(void)
{
    const char *product = profile_edition_product_string();

    if (product != NULL) {
        g_usb_string_table.product = product;
    }

    g_usb_device_descriptor.bcdDevice =
        (uint16_t)(USB_BCD_DEVICE_NKRO | ((g_run_state_flags & RUN_STATE_FLAG_6KRO) != 0 ? 1u : 0u));
    g_usb_device_descriptor.idProduct = (uint16_t)(USB_PID_BASE | (g_usb_link_flags & USB_LINK_MODE_MASK));

    if ((g_run_state_flags & RUN_STATE_FLAG_6KRO) != 0) {
        g_usb_kbd_report_descriptor = k_usb_hid_report_desc_kbd_6kro;
    } else {
        g_usb_kbd_report_descriptor = k_usb_hid_report_desc_kbd_nkro;
    }
}
STARTUP_STEP(STARTUP_USB_DESCRIPTORS, usb_descriptors_init);

/*
 * TinyUSB tud_init on the root port of the family, skipped if the stack is initialised
 * already: clears the device state, creates the event queue, runs the init functions of
 * the application (XInput) and HID class drivers and dcd_init of the port driver, enables
 * the USB interrupts and connects the device.
 */
void usb_stack_init(void)
{
    tud_init(hal_usb_root_port());
}
STARTUP_STEP(STARTUP_USB_STACK, usb_stack_init);
