/*
 * hal_usb.c - the USB device controller of the GD32F30x family around TinyUSB (interface:
 * hal/hal_usb.h).
 *
 * The device runs on the full-speed controller (USBD, TinyUSB root port 0) with its 48 MHz
 * clock taken from the system clock. The controller has no internal D+ pull-up: the pull-up
 * resistor is switched by a pin of wiring.h, which this file drives for TinyUSB
 * (dcd_connect, dcd_disconnect). The TinyUSB port driver is portable/st/stm32_fsdev
 * (TUSB_PORT_SRC in hal.mk).
 *
 * Depends on: the GD32F30x library, wiring.h of the board, TinyUSB.
 */
#include "gd32f30x.h"
#include "tusb.h"
#include "device/dcd.h"
#include "wiring.h"

#include "hal/hal_usb.h"

#define USB_ROOT_PORT   0u  /* the only device controller */

/* hal_usb_prepare enables the clock of this port only. */
_Static_assert(BOARD_USB_PULLUP_PORT == GPIOA, "USB pull-up switch on port A");

void USBD_HP_CAN0_TX_IRQHandler(void);
void USBD_LP_CAN0_RX0_IRQHandler(void);

/*
 * The serial-wire debug port keeps its two pins and the other JTAG pins become free for
 * general use (the pull-up switch may be one of them). The pull-up switch is driven low, the
 * USB clock is set to 48 MHz from the system clock and the controller clock enabled; the
 * device stays disconnected until tud_init connects it.
 */
void hal_usb_prepare(void)
{
    rcu_periph_clock_enable(RCU_GPIOA);
    gpio_pin_remap_config(GPIO_SWJ_SWDPENABLE_REMAP, ENABLE);

    gpio_init(BOARD_USB_PULLUP_PORT, GPIO_MODE_OUT_PP, GPIO_OSPEED_50MHZ, BOARD_USB_PULLUP_PIN);
    gpio_bit_reset(BOARD_USB_PULLUP_PORT, BOARD_USB_PULLUP_PIN);

    switch (rcu_clock_freq_get(CK_SYS)) {
    case 48000000u:
        rcu_usb_clock_config(RCU_CKUSB_CKPLL_DIV1);
        break;
    case 72000000u:
        rcu_usb_clock_config(RCU_CKUSB_CKPLL_DIV1_5);
        break;
    case 96000000u:
        rcu_usb_clock_config(RCU_CKUSB_CKPLL_DIV2);
        break;
    case 120000000u:
        rcu_usb_clock_config(RCU_CKUSB_CKPLL_DIV2_5);
        break;
    default:
        break;
    }
    rcu_periph_clock_enable(RCU_USBD);

    tud_disconnect();
}

/* The controller is full speed only. */
void hal_usb_limit_to_full_speed(void)
{
}

uint8_t hal_usb_root_port(void)
{
    return USB_ROOT_PORT;
}

/* D+ pull-up on: the host sees the device. */
void dcd_connect(uint8_t rhport)
{
    (void)rhport;
    gpio_bit_set(BOARD_USB_PULLUP_PORT, BOARD_USB_PULLUP_PIN);
}

/* D+ pull-up off: the host sees the device unplugged. */
void dcd_disconnect(uint8_t rhport)
{
    (void)rhport;
    gpio_bit_reset(BOARD_USB_PULLUP_PORT, BOARD_USB_PULLUP_PIN);
}

/* Interrupt 19 of the controller (high-priority endpoint transfers). */
void USBD_HP_CAN0_TX_IRQHandler(void)
{
    tud_int_handler(USB_ROOT_PORT);
}

/* Interrupt 20 of the controller (all other events). */
void USBD_LP_CAN0_RX0_IRQHandler(void)
{
    tud_int_handler(USB_ROOT_PORT);
}
