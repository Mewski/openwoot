/*
 * hal_usb.h - what the firmware does around the USB device controller outside TinyUSB: the
 * preparation of the controller before the stack starts, the speed limit, and the root port
 * the stack runs on.
 *
 * The rest belongs to the family and is not part of this interface: the TinyUSB port driver
 * and its binding (HAL_DEFS in hal.mk), the interrupt entries of the controller, and, where
 * the controller needs it, the board side of dcd_connect / dcd_disconnect.
 *
 * Depends on: nothing.
 */
#ifndef HAL_USB_H
#define HAL_USB_H

#include <stdint.h>

/* Switches on the clocks and pins of the USB controller and keeps the device invisible to
 * the host until TinyUSB connects it. Called once at start-up, before tud_init. */
void hal_usb_prepare(void);

/* The port will not enumerate faster than full speed. Called before the stack starts, when
 * the stored settings ask for it. Empty on a controller that is full speed only. */
void hal_usb_limit_to_full_speed(void);

/* The TinyUSB root port of the device controller, for tud_init and the interrupt entries. */
uint8_t hal_usb_root_port(void);

#endif /* HAL_USB_H */
