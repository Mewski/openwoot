/*
 * signals.c - the signals: built-in signals (index = enum rgb_signal_id) and host signals
 * (registered on their first subscription, values set by the host), subscription and delivery
 * of changed values to the subscribers.
 */
#include "rgb/signals/signals_internal.h"

#include "rgb/lightbar/lightbar.h"

rgb_signal_t g_rgb_signal_table[RGB_SIGNAL_COUNT];
rgb_host_signal_table_t g_rgb_host_signal_table;
uint8_t g_rgb_host_signals_active;

/* Sizes match stock firmware 2.14.1. */
_Static_assert(sizeof(g_rgb_signal_table) == 440, "g_rgb_signal_table");
_Static_assert(sizeof(g_rgb_host_signal_table) == 884, "g_rgb_host_signal_table");

float rgb_signal_subscribe(const SignalRef *signal_ref, uint32_t subscriber)
{
    if (signal_ref->is_builtin) {
        uint32_t id = signal_ref->id;
        rgb_signal_t *signal;

        if (id > RGB_SIGNAL_COUNT - 1) {
            return 0.0f;
        }
        signal = &g_rgb_signal_table[id];
        if (signal->subscriber_count < RGB_SIGNAL_SUBSCRIBER_MAX) {
            signal->subscribers[signal->subscriber_count] = subscriber;
            signal->subscriber_count++;
        }
        return signal->value;
    }

    /* Host-defined signal: find the entry with this id or append a new one. */
    uint32_t id = signal_ref->id;
    uint8_t count = g_rgb_host_signal_table.count;
    rgb_host_signal_t *host_signal = NULL;

    for (uint8_t i = 0; i < count; i++) {
        if (g_rgb_host_signal_table.entries[i].id == id) {
            host_signal = &g_rgb_host_signal_table.entries[i];
            break;
        }
    }
    if (host_signal == NULL) {
        if (count == RGB_HOST_SIGNAL_MAX) {
            return 0.0f;
        }
        g_rgb_host_signal_table.count = count + 1;
        host_signal = &g_rgb_host_signal_table.entries[count];
        host_signal->signal.value = 0.0f;
        host_signal->id = id;
        host_signal->signal.subscriber_count = 0;
    }
    if (host_signal->signal.subscriber_count < RGB_SIGNAL_SUBSCRIBER_MAX) {
        host_signal->signal.subscribers[host_signal->signal.subscriber_count] = subscriber;
        host_signal->signal.subscriber_count++;
    }
    return 0.0f;
}

void signals_notify(const rgb_signal_t *signal, const SignalRef *signal_ref, float value)
{
    for (uint8_t i = 0; i < signal->subscriber_count; i++) {
        uint32_t subscriber = signal->subscribers[i];

        if ((subscriber >> RGB_SUBSCRIBER_KIND_SHIFT) == RGB_SUBSCRIBER_KIND_LIGHTBAR) {
            rgb_lightbar_notify_signal(subscriber, signal_ref, value);
        }
    }
}

void rgb_set_signal_value(uint32_t signal_id, float value)
{
    rgb_signal_t *signal = &g_rgb_signal_table[signal_id];
    SignalRef signal_ref;

    signal->value = value;
    signal_ref.is_builtin = true;
    signal_ref.id = signal_id;
    signals_notify(signal, &signal_ref, value);
}

void signals_publish_if_changed(uint32_t signal_id, float value)
{
    if (value != g_rgb_signal_table[signal_id].value) {
        rgb_set_signal_value(signal_id, value);
    }
}

void signals_clear_subscribers(void)
{
    for (uint32_t i = 0; i < RGB_SIGNAL_COUNT; i++) {
        g_rgb_signal_table[i].subscriber_count = 0;
    }
    for (uint32_t i = 0; i < g_rgb_host_signal_table.count; i++) {
        g_rgb_host_signal_table.entries[i].signal.subscriber_count = 0;
    }
    g_rgb_host_signal_table.count = 0;
}

bool signals_host_values_active(void)
{
    return g_rgb_host_signals_active != 0;
}
