/*
 * hal_time.c - the two clocks of the GD32F30x family (interface: hal/hal_time.h): the 1 ms
 * tick on SysTick and the microsecond count on TIMER4 with its overflow interrupt.
 *
 * The core clock is 120 MHz (system_clock.c); TIMER4 runs at 120 MHz as well (APB1 at 60 MHz,
 * timers of a divided APB1 at twice that).
 *
 * Depends on: the GD32F30x library (rcu, timer) and CMSIS (SysTick_Config, NVIC_EnableIRQ).
 */
#include "gd32f30x.h"

#include "hal/hal_time.h"

#define HAL_TIME_TICK_CYCLES     120000u /* core clock cycles per millisecond */
#define HAL_TIME_MICROS_TIMER    TIMER4
#define HAL_TIME_MICROS_PRESCALE 119u    /* 120 MHz / 120 = 1 MHz */
#define HAL_TIME_MICROS_PERIOD   0xFFFEu /* auto-reload value of the 16-bit timer */

/* The register of inline/hal_time_inline.h is that of the peripheral library. */
_Static_assert(&HAL_TIME_MICROS_COUNTER == &TIMER_CNT(HAL_TIME_MICROS_TIMER), "HAL_TIME_MICROS_COUNTER");

volatile uint16_t g_hal_time_micros_high;

void TIMER4_IRQHandler(void);

/* SysTick_Config: reload 119999, the lowest priority (0xF0), counter 0, core clock,
 * interrupt and counter on. */
void hal_time_start_tick(void)
{
    (void)SysTick_Config(HAL_TIME_TICK_CYCLES);
}

/* TIMER4 counts up at 1 MHz from 0 to 0xFFFE; its update interrupt, at the default
 * priority 0, counts the overflows. */
void hal_time_start_micros(void)
{
    timer_parameter_struct timer;

    rcu_periph_clock_enable(RCU_TIMER4);

    timer.prescaler         = HAL_TIME_MICROS_PRESCALE;
    timer.alignedmode       = TIMER_COUNTER_EDGE;
    timer.counterdirection  = TIMER_COUNTER_UP;
    timer.period            = HAL_TIME_MICROS_PERIOD;
    timer.clockdivision     = TIMER_CKDIV_DIV1;
    timer.repetitioncounter = 0;
    timer_init(HAL_TIME_MICROS_TIMER, &timer);
    timer_update_event_enable(HAL_TIME_MICROS_TIMER);
    timer_interrupt_enable(HAL_TIME_MICROS_TIMER, TIMER_INT_UP);
    NVIC_EnableIRQ(TIMER4_IRQn);
    timer_enable(HAL_TIME_MICROS_TIMER);
}

/* Overflow of TIMER4: one more high half of the microsecond count. */
void TIMER4_IRQHandler(void)
{
    if ((TIMER_INTF(HAL_TIME_MICROS_TIMER) & TIMER_INTF_UPIF) != 0) {
        TIMER_INTF(HAL_TIME_MICROS_TIMER) &= ~TIMER_INTF_UPIF;
        g_hal_time_micros_high++;
    }
}
