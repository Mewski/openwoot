/*
 * protocol.h - public functions of the `protocol` module (transport of the host
 * configuration protocol) and the two lists through which the host commands are
 * registered. A handler is registered next to its definition with
 * PROTOCOL_FEATURE_COMMAND or PROTOCOL_REPORT_COMMAND; the dispatcher in protocol_core.c
 * looks the command number up in the list. The command numbers are in protocol_types.h.
 *
 * Depends on: core/registry.h, TinyUSB (HID callbacks), the nanopb messages of proto/, the
 * profile identifiers (profile/profile_types.h).
 */
#ifndef PROTOCOL_PROTOCOL_H_
#define PROTOCOL_PROTOCOL_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "tusb.h"

#include "core/registry.h"
#include "proto/settings.pb.h"
#include "profile/profile_types.h"
#include "protocol/protocol_types.h"
#include "protocol/protocol_data.h"

/* One feature command: its number (the command byte of the feature report) and the
 * handler that fills the response. */
typedef struct {
    uint32_t number;
    protocol_feature_handler_t handler;
} protocol_feature_command_t;

/* One OUT-report command: its number (the command byte of the report) and its handler. */
typedef struct {
    uint32_t number;
    protocol_report_handler_t handler;
} protocol_report_command_t;

REGISTRY_DECLARE(protocol_feature_commands, protocol_feature_command_t);
REGISTRY_DECLARE(protocol_report_commands, protocol_report_command_t);

/* Registers `handler` (protocol_feature_handler_t) as feature command `number`. */
#define PROTOCOL_FEATURE_COMMAND(number, handler) \
    REGISTRY_ENTRY(protocol_feature_commands, protocol_feature_command_t, handler) = { (number), (handler) }

/* Registers `handler` (protocol_report_handler_t) as OUT-report command `number`. */
#define PROTOCOL_REPORT_COMMAND(number, handler) \
    REGISTRY_ENTRY(protocol_report_commands, protocol_report_command_t, handler) = { (number), (handler) }

/* Profile id addressed by the flags byte of an OUT report (bits 5:1 index, bits 7:6 namespace). */
static inline uint16_t protocol_report_profile_id(uint8_t flags)
{
    return PROFILE_ID_RAW((flags & PROTOCOL_REPORT_FLAG_INDEX_MASK) >> PROTOCOL_REPORT_FLAG_INDEX_SHIFT,
                          flags >> PROTOCOL_REPORT_FLAG_NS_SHIFT);
}

void protocol_handle_feature_command(const protocol_feature_cmd_t *entry);
void protocol_handle_report_command(const uint8_t *report);
void protocol_delete_last_profile(uint32_t arg, protocol_response_t *response, uint32_t unused, uint32_t profile_id);
void protocol_send_profiles_changed_event(void);
void protocol_flush_tx_ring(void);
/* The two TinyUSB HID callbacks of this unit (protocol_core.c) are declared by
 * class/hid/hid_device.h (through tusb.h), with the signatures of the TinyUSB copy in
 * third_party/tinyusb (tud_hid_set_report_cb returns a value there):
 *   void tud_hid_report_complete_cb(uint8_t instance, uint8_t const *report, uint8_t len);
 *   bool tud_hid_set_report_cb(uint8_t instance, uint8_t report_id, hid_report_type_t report_type,
 *                              uint8_t const *buffer, uint16_t bufsize); */
void protocol_queue_report(const uint8_t *data, uint32_t len);
void protocol_send_event(const Event *event);

#endif /* PROTOCOL_PROTOCOL_H_ */
