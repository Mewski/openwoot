/*
 * hal_time.c - the two clocks of the GD32F4xx family (interface: hal/hal_time.h): the 1 ms
 * tick on SysTick and the microsecond count on TIMER4.
 *
 * The core clock is 200 MHz (system_clock.c); the timers run at the same 200 MHz, because the
 * set-up of the sensor scan selects the timer clock multiplier before hal_time_start_micros
 * runs.
 *
 * Depends on: the GD32F4xx library (rcu, timer) and CMSIS (SysTick_Config).
 */
#include "gd32f4xx.h"

#include "hal/hal_time.h"

#define HAL_TIME_TICK_CYCLES     200000u     /* core clock cycles per millisecond */
#define HAL_TIME_MICROS_TIMER    TIMER4
#define HAL_TIME_MICROS_PRESCALE 199u        /* 200 MHz / 200 = 1 MHz */
#define HAL_TIME_MICROS_PERIOD   0xFFFFFFFEu /* auto-reload value of the 32-bit timer */

/* The register of inline/hal_time_inline.h is that of the peripheral library. */
_Static_assert(&HAL_TIME_MICROS_COUNTER == &TIMER_CNT(HAL_TIME_MICROS_TIMER), "HAL_TIME_MICROS_COUNTER");

/* SysTick_Config: reload 199999, the lowest priority (0xF0), counter 0, core clock,
 * interrupt and counter on. */
void hal_time_start_tick(void)
{
    (void)SysTick_Config(HAL_TIME_TICK_CYCLES);
}

/* TIMER4 counts up at 1 MHz over the whole 32-bit range; no interrupt. */
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
    timer_enable(HAL_TIME_MICROS_TIMER);
}
