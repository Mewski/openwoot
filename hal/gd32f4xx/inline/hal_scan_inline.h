/*
 * hal_scan_inline.h - the per-sample functions of the sensor scan on a GD32F4xx
 * (interface: hal/hal_scan.h).
 *
 * ADC0 converts the inputs; channel 1 of TIMER1 (1 MHz, single pulse) triggers each
 * conversion; a result word carries the reading in its low half and no reference. The
 * address lines are driven with one store of a precomputed set/reset value into the
 * bit operate register of their port; hal_scan_init in hal_scan.c builds the values from
 * the board's address line pins.
 *
 * Depends on: the board's wiring.h, for its numeric BOARD_MUX_ADDR_PORT_BASE only (a wiring.h
 * holds nothing but macros, so including it pulls in no library header, and this header
 * expands no constant of it that is written in the library's vocabulary);
 * g_hal_scan_address_values (hal_scan.c). The registers are defined here; hal_scan.c checks
 * each address against the peripheral library.
 */
#ifndef HAL_SCAN_INLINE_H
#define HAL_SCAN_INLINE_H

#include <stdint.h>

#include "wiring.h"

#define HAL_SCAN_ADC_STAT        (*(volatile uint32_t *)0x40012000u) /* ADC_STAT(ADC0) */
#define HAL_SCAN_ADC_CTL1        (*(volatile uint32_t *)0x40012008u) /* ADC_CTL1(ADC0) */
#define HAL_SCAN_TIMER_CTL0      (*(volatile uint32_t *)0x40000000u) /* TIMER_CTL0(TIMER1) */
#define HAL_SCAN_TIMER_CNT       (*(volatile uint32_t *)0x40000024u) /* TIMER_CNT(TIMER1) */
#define HAL_SCAN_TIMER_CH1CV     (*(volatile uint32_t *)0x40000038u) /* TIMER_CH1CV(TIMER1) */
/* GPIO_BOP(BOARD_MUX_ADDR_PORT): the bit operate register of the port of the address lines. */
#define HAL_SCAN_ADDRESS_BOP     (*(volatile uint32_t *)(BOARD_MUX_ADDR_PORT_BASE + 0x18u))

#define HAL_SCAN_ADC_STAT_EOC    0x00000002u /* ADC_STAT_EOC: end of conversion */
#define HAL_SCAN_ADC_ETSRC       0x0F000000u /* ADC_CTL1_ETSRC: trigger source of the routine sequence */
#define HAL_SCAN_ADC_ETSRC_TIMER 0x03000000u /* ADC_EXTTRIG_REGULAR_T1_CH1 */
#define HAL_SCAN_TIMER_CEN       0x00000001u /* TIMER_CTL0_CEN: counter on */

/* Per address: the value for HAL_SCAN_ADDRESS_BOP that sets the lines of the address's one
 * bits and resets the others. */
extern uint32_t g_hal_scan_address_values[];

__attribute__((always_inline))
static inline void hal_scan_ack_conversion(void)
{
    HAL_SCAN_ADC_STAT &= ~HAL_SCAN_ADC_STAT_EOC;
}

__attribute__((always_inline))
static inline uint16_t hal_scan_sample_value(uint32_t word)
{
    return (uint16_t)word;
}

__attribute__((always_inline))
static inline uint16_t hal_scan_sample_reference(uint32_t word)
{
    (void)word;
    return 0u;
}

__attribute__((always_inline))
static inline void hal_scan_select_address(uint32_t address)
{
    HAL_SCAN_ADDRESS_BOP = g_hal_scan_address_values[address];
}

/* The timer restarts from 0 with the delay as compare value, and the converter is
 * triggered by it again (hal_scan_end_cycle had switched the trigger source away). */
__attribute__((always_inline))
static inline void hal_scan_trigger_after(uint32_t delay_us)
{
    HAL_SCAN_TIMER_CTL0 &= ~HAL_SCAN_TIMER_CEN;
    HAL_SCAN_TIMER_CNT = 0u;
    HAL_SCAN_TIMER_CH1CV = delay_us;
    HAL_SCAN_ADC_CTL1 &= ~HAL_SCAN_ADC_ETSRC;
    HAL_SCAN_ADC_CTL1 |= HAL_SCAN_ADC_ETSRC_TIMER;
    HAL_SCAN_TIMER_CTL0 |= HAL_SCAN_TIMER_CEN;
}

#endif /* HAL_SCAN_INLINE_H */
