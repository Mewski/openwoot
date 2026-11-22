/*
 * hal_scan.h - the converter side of the sensor scan.
 *
 * The firmware's view: the sensors sit behind BOARD_MUX_COUNT inputs (one multiplexer
 * each) that share one set of address lines. A conversion converts either one input or
 * all inputs at the selected address. It is started a given number of microseconds
 * after it was requested; its results arrive as 32-bit words in a buffer of the
 * firmware, and then hal_scan_conversion_isr runs. The scan state machine (which input
 * and address come next, what a reading means) is the firmware's; this interface only
 * moves the hardware.
 *
 * The functions the scan interrupt calls for every sample are inline, so that they cost
 * no call: each family implements them in inline/hal_scan_inline.h. Which converter,
 * timer and transfer channel a family uses, and how it orders its register writes, is
 * its own business; the order is that of the stock firmware 2.14.1 of the boards of the
 * family.
 *
 * Depends on: the board's board.h (dimensions, converter numbers) and wiring.h (pins,
 * converter channels), seen by the family implementation only; the family's inline header.
 * The firmware provides two call-backs: hal_scan_conversion_isr and hal_scan_wait_ms.
 */
#ifndef HAL_SCAN_H
#define HAL_SCAN_H

#include <stdint.h>

/* Configures the inputs (all off), the address lines (address 0), the converter, the
 * trigger and the transport of the results into `samples[BOARD_MUX_COUNT]`, and enables
 * the conversion interrupt. One result per conversion; no conversion is started. Waits
 * through hal_scan_wait_ms while the converter powers up. */
void hal_scan_init(uint32_t *samples);

/* Call-back, provided by the firmware: runs after every conversion, in interrupt
 * context. Its first statement is hal_scan_ack_conversion(). */
void hal_scan_conversion_isr(void);

/* Call-back, provided by the firmware: returns after `ms` ticks of the 1 ms tick
 * (hal/hal_time.h). It is part of the interface because the millisecond count belongs to the
 * firmware (hal_time_tick_isr keeps it), so the HAL has no clock of its own to wait on; the
 * stock firmware 2.14.1 waits 1 ms on that count while the converter powers up. Only
 * hal_scan_init calls it, before the scan runs, with the tick running. */
void hal_scan_wait_ms(uint32_t ms);

/* Acknowledges the conversion interrupt. */
static inline void hal_scan_ack_conversion(void);

/* The sensor reading carried by a result word. */
static inline uint16_t hal_scan_sample_value(uint32_t word);

/* The zero-field reference that the hardware converted together with the reading in
 * this result word; 0 if the hardware measures no reference. */
static inline uint16_t hal_scan_sample_reference(uint32_t word);

/* Drives `address` (0 .. BOARD_MUX_POSITIONS - 1) on the shared address lines. */
static inline void hal_scan_select_address(uint32_t address);

/* Single-input scanning: switches input `previous` off and input `next` on; the next
 * conversion converts `next`. */
void hal_scan_select_input(uint32_t previous, uint32_t next);

/* Switches to single-input scanning: all inputs off, then input 0 on (input `previous`
 * is switched off once more on its own), address 0, one result per conversion. */
void hal_scan_enter_single(uint32_t previous);

/* Switches to parallel scanning: all inputs on, `address` selected, one conversion
 * delivers BOARD_MUX_COUNT results; result n is the reading of input `input_order[n]`. */
void hal_scan_enter_parallel(const uint8_t *input_order, uint32_t address);

/* The switch to parallel scanning in the middle of the start-up sweep, in the register
 * order the sweep uses: the converter and the transport first, then, if `input_order`
 * is not NULL, the inputs in that order. NULL: the inputs are set up in parallel order
 * already (a previous hal_scan_enter_parallel). The address lines are left as they are. */
void hal_scan_sweep_enter_parallel(const uint8_t *input_order);

/* Starts one conversion `delay_us` microseconds after the call. */
static inline void hal_scan_trigger_after(uint32_t delay_us);

/* Starts the first conversion of a cycle when the stopwatch started by
 * hal_scan_end_cycle reaches `ticks` - 1 microseconds, or at once if that point has
 * passed. */
void hal_scan_trigger_at(uint16_t ticks);

/* Ends a cycle: no further conversion is triggered, and the stopwatch that
 * hal_scan_trigger_at measures from starts at 0. Once `period_us` microseconds have
 * passed, every hal_scan_trigger_at starts its conversion at once. */
void hal_scan_end_cycle(uint32_t period_us);

#include "hal_scan_inline.h"

#endif /* HAL_SCAN_H */
