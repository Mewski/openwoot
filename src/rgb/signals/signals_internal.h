/*
 * signals_internal.h - state of feature `signals`, shared by its source files: the built-in
 * signals, the host signals and the delivery of a changed value to the subscribers.
 *
 * The layouts match stock firmware 2.14.1 (the host addresses the host signals by their index
 * in the table).
 *
 * Depends on rgb/signals/signals.h.
 */
#ifndef RGB_SIGNALS_SIGNALS_INTERNAL_H
#define RGB_SIGNALS_SIGNALS_INTERNAL_H

#include <stddef.h>
#include <stdint.h>

#include "rgb/signals/signals.h"

#define RGB_SIGNAL_SUBSCRIBER_MAX  8
#define RGB_HOST_SIGNAL_MAX        20

/* One signal: value and the subscribers notified on a change. */
typedef struct {
    float    value;                                  /* 0x00 last value */
    uint8_t  subscriber_count;                       /* 0x04 0..8, cleared by signals_clear_subscribers */
    uint8_t  _pad_05[3];
    uint32_t subscribers[RGB_SIGNAL_SUBSCRIBER_MAX]; /* 0x08 subscriber ids */
} rgb_signal_t;

_Static_assert(sizeof(rgb_signal_t) == 40, "rgb_signal_t size");
_Static_assert(offsetof(rgb_signal_t, subscriber_count) == 0x04, "rgb_signal_t.subscriber_count");
_Static_assert(offsetof(rgb_signal_t, subscribers) == 0x08, "rgb_signal_t.subscribers");

/* Host-defined signal, registered by its first subscriber, value set by report command 31. */
typedef struct {
    uint32_t     id;     /* 0x00 opaque id from the signal reference */
    rgb_signal_t signal; /* 0x04 value @4, subscriber_count @8, subscribers @0xC */
} rgb_host_signal_t;

_Static_assert(sizeof(rgb_host_signal_t) == 44, "rgb_host_signal_t size");
_Static_assert(offsetof(rgb_host_signal_t, signal.value) == 0x04, "rgb_host_signal_t.value");
_Static_assert(offsetof(rgb_host_signal_t, signal.subscriber_count) == 0x08, "rgb_host_signal_t.subscriber_count");
_Static_assert(offsetof(rgb_host_signal_t, signal.subscribers) == 0x0C, "rgb_host_signal_t.subscribers");

typedef struct {
    uint8_t           count;                        /* 0x000 registered host signals, 0..20 */
    uint8_t           _pad_001[3];
    rgb_host_signal_t entries[RGB_HOST_SIGNAL_MAX]; /* 0x004 in order of registration (the host addresses them by index) */
} rgb_host_signal_table_t;

_Static_assert(sizeof(rgb_host_signal_table_t) == 884, "rgb_host_signal_table_t size");
_Static_assert(offsetof(rgb_host_signal_table_t, entries) == 0x004, "rgb_host_signal_table_t.entries");

extern rgb_signal_t g_rgb_signal_table[RGB_SIGNAL_COUNT]; /* built-in signals, index = enum rgb_signal_id */
extern rgb_host_signal_table_t g_rgb_host_signal_table;   /* host signals registered by the subscribers */
extern uint8_t g_rgb_host_signals_active;                 /* set once the host listed or set the host signals; never cleared */

/* Delivers a new value of `signal` (named by signal_ref) to each of its subscribers. */
void signals_notify(const rgb_signal_t *signal, const SignalRef *signal_ref, float value);

#endif /* RGB_SIGNALS_SIGNALS_INTERNAL_H */
