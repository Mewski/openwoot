/*
 * hal_irq.h - interrupt masking.
 *
 * A short section of code that must not be interrupted saves the mask, runs and restores
 * it; the saved state is an opaque word that is only passed back to hal_irq_restore.
 *
 * Both functions are inline, so that masking costs no call: each family implements them in
 * inline/hal_irq_inline.h.
 *
 * Depends on: nothing but the family's inline header.
 */
#ifndef HAL_IRQ_H
#define HAL_IRQ_H

#include <stdint.h>

/* Masks interrupts and returns the state the mask had before. */
static inline uint32_t hal_irq_save(void);

/* Unmasks interrupts if `state`, a value returned by hal_irq_save, says that they were
 * enabled at that time; otherwise leaves them masked. */
static inline void hal_irq_restore(uint32_t state);

#include "hal_irq_inline.h"

#endif /* HAL_IRQ_H */
