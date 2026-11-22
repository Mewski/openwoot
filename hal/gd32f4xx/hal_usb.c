/*
 * hal_usb.c - the USB device controller of the GD32F4xx family around TinyUSB (interface:
 * hal/hal_usb.h).
 *
 * The device runs on the high-speed controller (USBHS, TinyUSB root port 1) with an external
 * transceiver on the ULPI pins; those pins and the reset line of the transceiver are pins of
 * wiring.h. The TinyUSB port driver is portable/synopsys/dwc2 (TUSB_PORT_SRC in hal.mk); it
 * soft-connects and soft-disconnects through the controller itself, and reads the speed limit
 * from g_usb_high_speed_enabled when the stack starts.
 *
 * Depends on: the GD32F4xx library, wiring.h of the board, TinyUSB.
 */
#include "gd32f4xx.h"
#include "tusb.h"
#include "wiring.h"

#include "hal/hal_usb.h"

#define USB_ROOT_PORT   1u  /* the high-speed controller */

/* The alternate function is set once per port, as in stock firmware 2.14.1, for STP, DIR and
 * NXT together and for CK and D0 together. */
_Static_assert(BOARD_ULPI_DIR_PORT == BOARD_ULPI_STP_PORT && BOARD_ULPI_NXT_PORT == BOARD_ULPI_STP_PORT,
               "BOARD_ULPI_DIR_PORT, BOARD_ULPI_NXT_PORT (wiring.h): the port of STP");
_Static_assert(BOARD_ULPI_D0_PORT == BOARD_ULPI_CK_PORT, "BOARD_ULPI_D0_PORT (wiring.h): the port of CK");

/* Non-zero: the DWC2 port driver enumerates at high speed; 0: at full speed on the
 * high-speed transceiver (read by dcd_init). */
uint8_t g_usb_high_speed_enabled = 0xFF;

void USBHS_IRQHandler(void);

/* One group of ULPI pins as alternate function, push-pull, maximum speed. */
static void usb_ulpi_pins_mode_set(uint32_t port, uint32_t pins)
{
    gpio_mode_set(port, GPIO_MODE_AF, GPIO_PUPD_NONE, pins);
    gpio_output_options_set(port, GPIO_OTYPE_PP, GPIO_OSPEED_MAX, pins);
}

/*
 * The transceiver is held in reset while the twelve ULPI pins are switched to alternate
 * function 10 and the USB clocks are enabled, then released. The controller is
 * soft-disconnected before that, so that the host sees no device until tud_init connects.
 */
void hal_usb_prepare(void)
{
    rcu_periph_clock_enable(RCU_SYSCFG);
    rcu_periph_clock_enable(RCU_GPIOA);
    rcu_periph_clock_enable(RCU_GPIOB);
    rcu_periph_clock_enable(RCU_GPIOC);

    /* reset line of the transceiver: output push-pull, low */
    gpio_mode_set(BOARD_USB_PHY_RESET_PORT, GPIO_MODE_OUTPUT, GPIO_PUPD_NONE, BOARD_USB_PHY_RESET_PIN);
    gpio_output_options_set(BOARD_USB_PHY_RESET_PORT, GPIO_OTYPE_PP, GPIO_OSPEED_50MHZ, BOARD_USB_PHY_RESET_PIN);
    gpio_bit_reset(BOARD_USB_PHY_RESET_PORT, BOARD_USB_PHY_RESET_PIN);

    /* ULPI pins */
    usb_ulpi_pins_mode_set(BOARD_ULPI_STP_PORT, BOARD_ULPI_STP_PIN);
    usb_ulpi_pins_mode_set(BOARD_ULPI_CK_PORT, BOARD_ULPI_CK_PIN);
    usb_ulpi_pins_mode_set(BOARD_ULPI_NXT_PORT, BOARD_ULPI_NXT_PIN);
    usb_ulpi_pins_mode_set(BOARD_ULPI_DIR_PORT, BOARD_ULPI_DIR_PIN);
    usb_ulpi_pins_mode_set(BOARD_ULPI_D1_D7_PORT, BOARD_ULPI_D1_D7_PINS);
    usb_ulpi_pins_mode_set(BOARD_ULPI_D0_PORT, BOARD_ULPI_D0_PIN);
    gpio_af_set(BOARD_ULPI_STP_PORT, GPIO_AF_10, BOARD_ULPI_STP_PIN | BOARD_ULPI_DIR_PIN | BOARD_ULPI_NXT_PIN);
    gpio_af_set(BOARD_ULPI_CK_PORT, GPIO_AF_10, BOARD_ULPI_D0_PIN | BOARD_ULPI_CK_PIN);
    gpio_af_set(BOARD_ULPI_D1_D7_PORT, GPIO_AF_10, BOARD_ULPI_D1_D7_PINS);

    /* USB clocks, soft disconnect (DCTL.SDIS), transceiver out of reset */
    rcu_periph_clock_enable(RCU_USBHSULPI);
    rcu_periph_clock_enable(RCU_USBHS);
    tud_disconnect();
    gpio_bit_set(BOARD_USB_PHY_RESET_PORT, BOARD_USB_PHY_RESET_PIN);
}

/* dcd_init then selects full speed on the high-speed transceiver (DCFG.DSPD = 1). */
void hal_usb_limit_to_full_speed(void)
{
    g_usb_high_speed_enabled = 0;
}

uint8_t hal_usb_root_port(void)
{
    return USB_ROOT_PORT;
}

/* Interrupt 77 of the high-speed controller. */
void USBHS_IRQHandler(void)
{
    tud_int_handler(USB_ROOT_PORT);
}
