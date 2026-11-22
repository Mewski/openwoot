/*
 * hal_irq_inline.h - interrupt masking on a Cortex-M4 core (interface: hal/hal_irq.h).
 *
 * The mask is PRIMASK: 0 when interrupts are enabled, 1 when they are masked. The saved
 * state of hal_irq_save is the PRIMASK value.
 *
 * Depends on: nothing; the functions are single core instructions.
 */
#ifndef HAL_IRQ_INLINE_H
#define HAL_IRQ_INLINE_H

#include <stdint.h>

__attribute__((always_inline))
static inline uint32_t hal_irq_save(void)
{
    uint32_t state;

    __asm__ volatile("mrs %0, primask" : "=r"(state));
    __asm__ volatile("cpsid i" ::: "memory");
    return state;
}

__attribute__((always_inline))
static inline void hal_irq_restore(uint32_t state)
{
    if (state == 0) {
        __asm__ volatile("cpsie i" ::: "memory");
    }
}

#endif /* HAL_IRQ_INLINE_H */
