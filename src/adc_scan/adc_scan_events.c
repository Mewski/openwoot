/*
 * adc_scan_events.c - key event ring between the ADC interrupt (writer) and the main loop (reader).
 *
 * Ring convention (30 entries): g_adc_scan_event_head = index of the newest entry,
 * g_adc_scan_event_tail = index of the next entry to read, both -1 while the ring is empty. The ring
 * is full when head + 1 == tail, or head == 29 and tail == 0. At the end of a scan cycle the
 * interrupt reserves one more slot as end-of-cycle marker (g_adc_scan_event_scan_end).
 */
#include "adc_scan/adc_scan.h"

#include "hal/hal_irq.h"

#define ADC_SCAN_EVENT_LAST_INDEX (ADC_SCAN_EVENT_QUEUE_LEN - 1)

/* Claim the next ring slot for the writer. Returns the slot index (the new head), or -1 when the
 * ring is full. */
int adc_scan_event_reserve(void)
{
    if (g_adc_scan_event_tail == ADC_SCAN_EVENT_INDEX_NONE ||
        g_adc_scan_event_head == ADC_SCAN_EVENT_INDEX_NONE) {
        /* empty ring: the first entry goes to slot 0 */
        g_adc_scan_event_head = 0;
        g_adc_scan_event_tail = 0;
        return g_adc_scan_event_head;
    }

    if ((g_adc_scan_event_tail == 0 && g_adc_scan_event_head == ADC_SCAN_EVENT_LAST_INDEX) ||
        g_adc_scan_event_head + 1 == g_adc_scan_event_tail) {
        return -1; /* full */
    }

    if (g_adc_scan_event_head == ADC_SCAN_EVENT_LAST_INDEX && g_adc_scan_event_tail != 0) {
        g_adc_scan_event_head = 0;
        return g_adc_scan_event_head;
    }

    g_adc_scan_event_head = (int8_t)(g_adc_scan_event_head + 1);
    return g_adc_scan_event_head;
}

/* Queue one key event for the main loop. Called from the ADC interrupt. */
void adc_scan_event_push(uint8_t key_id, analog_key_state_t *key_state, uint16_t travel)
{
    /* a full ring drops the event */
    int index = adc_scan_event_reserve();
    if (index == -1) {
        return;
    }

    adc_scan_event_t *event = &g_adc_scan_event_queue[index];
    event->key_id = key_id;
    event->key_state = key_state;
    event->travel = travel;
}

/* Take the oldest event out of the ring (main loop; interrupts are masked while the indices are
 * read and updated). Returns ADC_SCAN_POP_EVENT with *event_out filled, ADC_SCAN_POP_EMPTY, or
 * ADC_SCAN_POP_CYCLE_END when the read index reached the end-of-cycle marker; in that case
 * g_adc_scan_cycle_busy is cleared. */
int adc_scan_event_pop(adc_scan_event_t *event_out)
{
    uint32_t irq_state = hal_irq_save();

    if (g_adc_scan_event_tail == ADC_SCAN_EVENT_INDEX_NONE) {
        hal_irq_restore(irq_state);
        return ADC_SCAN_POP_EMPTY;
    }

    int tail = g_adc_scan_event_tail;

    if (g_adc_scan_event_scan_end == tail) {
        /* the marker slot itself stays in the ring; adc_scan_start_cycle resets the indices */
        hal_irq_restore(irq_state);
        g_adc_scan_cycle_busy = 0u;
        return ADC_SCAN_POP_CYCLE_END;
    }

    if (g_adc_scan_event_head == tail) {
        /* last entry taken: ring empty */
        g_adc_scan_event_tail = ADC_SCAN_EVENT_INDEX_NONE;
        g_adc_scan_event_head = ADC_SCAN_EVENT_INDEX_NONE;
    } else if (g_adc_scan_event_tail == ADC_SCAN_EVENT_LAST_INDEX) {
        g_adc_scan_event_tail = 0;
    } else {
        g_adc_scan_event_tail = (int8_t)(g_adc_scan_event_tail + 1);
    }

    const adc_scan_event_t *event = &g_adc_scan_event_queue[tail];
    uint16_t travel = event->travel;
    analog_key_state_t *key_state = event->key_state;
    uint8_t key_id = event->key_id;
    event_out->travel = travel;
    event_out->key_state = key_state;
    event_out->key_id = key_id;

    hal_irq_restore(irq_state);
    return ADC_SCAN_POP_EVENT;
}
