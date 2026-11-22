/*
 * signals_protocol.c - host commands of feature `signals`:
 *
 *   feature command 65     protocol_cmd_get_host_signals   HostSignalList
 *   OUT-report command 31  protocol_cmd_set_host_signals   HostSignalValues
 *
 * Subscribers register the host signals they use in g_rgb_host_signal_table; after the light bar
 * has bound its entries it sends event 2, the host then reads the {index, id} list (command 65)
 * and sets values by index (command 31).
 *
 * Depends on the protocol module (registration, report layouts), the profile lock mode and
 * nanopb.
 */
#include <string.h>

#include "pb_decode.h"
#include "pb_encode.h"

#include "profile/profile_data.h"
#include "protocol/protocol.h"
#include "rgb/signals/signals_internal.h"

_Static_assert(sizeof(((HostSignalList *)0)->signals) / sizeof(HostSignalListEntry) == RGB_HOST_SIGNAL_MAX,
               "HostSignalList holds every host signal");

/*
 * Feature command 65: lists the host signals in use as {table index, id} pairs. The list is
 * empty while the device is in a locked mode. A successful answer also enables the host-signal
 * effects (g_rgb_host_signals_active). arg is unused.
 */
static void protocol_cmd_get_host_signals(uint32_t arg, protocol_response_t *response)
{
    HostSignalList list;
    pb_ostream_t stream;

    memset(&list, 0, sizeof(list));
    stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);

    if (!PROFILE_LOCK_MODE_IS_LOCKED(g_profile_lock_mode)) {
        uint8_t count = g_rgb_host_signal_table.count;

        list.signals_count = count;
        for (uint8_t i = 0; i < count; i++) {
            list.signals[i].index = i;
            list.signals[i].id = g_rgb_host_signal_table.entries[i].id;
        }
    }

    if (pb_encode(&stream, &HostSignalList_msg, &list)) {
        response->length = (uint16_t)stream.bytes_written;
        g_rgb_host_signals_active = 1;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->length = 0;
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_HOST_SIGNALS, protocol_cmd_get_host_signals);

/*
 * OUT-report command 31: the host sets values of host signals, addressed by their index in
 * g_rgb_host_signal_table. Every value that differs from the stored one is stored and delivered
 * to the subscribers of the signal. Ignored in a locked mode. No response.
 */
static void protocol_cmd_set_host_signals(const uint8_t *report)
{
    const protocol_report_plain_t *request = (const protocol_report_plain_t *)report;
    HostSignalValues values;
    pb_istream_t stream;

    memset(&values, 0, sizeof(values));
    if (PROFILE_LOCK_MODE_IS_LOCKED(g_profile_lock_mode)) {
        return;
    }

    stream = pb_istream_from_buffer(request->payload, request->length);
    if (!pb_decode(&stream, &HostSignalValues_msg, &values)) {
        return;
    }

    g_rgb_host_signals_active = 1;

    for (uint8_t i = 0; i < values.values_count; i++) {
        uint8_t signal_index = values.values[i].index;
        float value = values.values[i].value;
        rgb_host_signal_t *entry;
        SignalRef signal_ref;

        if (signal_index >= g_rgb_host_signal_table.count) {
            continue;
        }
        entry = &g_rgb_host_signal_table.entries[signal_index];
        if (value == entry->signal.value) {
            continue;
        }
        entry->signal.value = value;

        signal_ref.is_builtin = false;
        signal_ref.id = entry->id;
        signals_notify(&entry->signal, &signal_ref, value);
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_HOST_SIGNALS, protocol_cmd_set_host_signals);
