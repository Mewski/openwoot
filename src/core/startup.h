/*
 * core: the start-up sequence. Every module registers its own start-up steps with
 * STARTUP_STEP next to their functions; the start-up runner of src/app/ calls them in
 * ascending order number before the main loop. The constants below are the order of
 * the core steps and therefore the readable start-up sequence; it is the order of the
 * stock firmware 2.14.1 and hardware-visible (register writes, file reads), so a step
 * never moves without a reason recorded in its module. A feature places its steps
 * relative to a core slot in its own header (for example
 * STARTUP_LIGHTBAR_LEDS = STARTUP_LED_BUS + 1); the gaps of 10 leave room for that.
 *
 * Depends on: core/registry.h.
 */
#ifndef CORE_STARTUP_H_
#define CORE_STARTUP_H_

#include <stdint.h>

#include "core/registry.h"

/* One start-up step: its slot in the sequence and the function that performs it. */
typedef struct {
    uint16_t order;
    void (*run)(void);
} startup_step_t;

REGISTRY_DECLARE(startup_steps, startup_step_t);

/* Registers `function` (void (void)) as the start-up step in slot `order`. */
#define STARTUP_STEP(order, function) \
    REGISTRY_ENTRY(startup_steps, startup_step_t, function) = { (order), (function) }

/* Slots of the core steps, in the order in which they run. */
enum startup_order {
    /* clocks, tick, USB port */
    STARTUP_TIME_TICK              = 10,  /* hal_time_start_tick: 1 ms tick, its interrupt runs from here on */
    STARTUP_SYSTEM                 = 20,  /* hal_system_init: port and DMA clocks */
    STARTUP_USB_PREPARE            = 30,  /* hal_usb_prepare: USB port and clocks prepared, device disconnected */
    STARTUP_SOFT_TIMERS            = 40,  /* soft_timers_init */

    /* storage and factory information */
    STARTUP_NOR_FLASH              = 50,  /* nor_flash_init: bus of the external flash */
    STARTUP_STORAGE                = 60,  /* storage_init: mount, format if needed */
    STARTUP_FIRST_BOOT_PROFILES    = 70,  /* storage_create_first_boot_profiles */
    STARTUP_LOCK_MODE              = 80,  /* device_info_load_lock_mode */
    STARTUP_SERIAL_MIRROR          = 90,  /* device_serial_mirror_init */

    /* settings */
    STARTUP_SETTINGS               = 100, /* settings_load_and_apply */
    STARTUP_LAYOUT_VARIANT         = 110, /* layout_select_variant */
    STARTUP_USB_MODE               = 120, /* settings_apply_usb_mode */
    STARTUP_PROFILE_COUNTS         = 130, /* profile_counts_init */

    /* key records and switch profiles */
    STARTUP_KEY_ACTUATION_RESET    = 140, /* analog_key_actuation_reset */
    STARTUP_SWITCH_PROFILES        = 150, /* analog_switch_profiles_init */

    /* hall-sensor scan */
    STARTUP_SCAN_HARDWARE          = 160, /* hal_scan_init: multiplexer pins, trigger timer, ADC */
    STARTUP_TIME_MICROS            = 170, /* hal_time_start_micros */
    STARTUP_KEY_RECORDS            = 180, /* analog_key_records_init */
    STARTUP_SCAN_COLUMN_TABLE      = 190, /* adc_scan_column_table_init */
    STARTUP_SCAN_SWEEP             = 200, /* adc_scan_start_sweep: the scan interrupt runs from here on */

    /* global settings, key task */
    STARTUP_GLOBAL_SETTINGS        = 210, /* global_settings_load */
    STARTUP_KEYS_TASK              = 220, /* keys_task_init */

    /* lighting */
    STARTUP_LED_BUS                = 230, /* hal_led_bus_init */
    STARTUP_LED_MATRIX             = 240, /* led_matrix_init: driver reset, stays in shutdown */
    STARTUP_FRAME_PAGES            = 250, /* rgb_frame_pages_init */
    STARTUP_PROFILE_ACTIVATE       = 260, /* rgb_start_and_activate_profile */

    /* USB */
    STARTUP_USB_SPEED              = 270, /* hal_usb_limit_to_full_speed per the device settings */
    STARTUP_USB_SERIAL_STRING      = 280, /* usb_serial_string_init */
    STARTUP_USB_DESCRIPTORS        = 290, /* usb_descriptors_init */
    STARTUP_USB_STACK              = 300, /* usb_stack_init: the USB interrupt runs from here on */

    /* last settings, first scan cycle */
    STARTUP_REBOUND_FILTER         = 310, /* rebound_filter_option_init */
    STARTUP_SCAN_CYCLE             = 320, /* adc_scan_start_cycle: waits for the end of the sweep */
};

#endif /* CORE_STARTUP_H_ */
