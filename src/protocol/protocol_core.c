/*
 * protocol_core.c - transport of the host configuration protocol (HID interface 2):
 * IN report queue, unsolicited events, dispatch of feature and OUT-report commands through
 * the registered lists of protocol.h and the TinyUSB HID callbacks that receive feature /
 * OUT reports (plus the keyboard LED report).
 */
#include <string.h>

#include "tusb.h"
#include "pb_encode.h"

#include "protocol/protocol.h"
#include "hal/hal_irq.h"
#include "profile/profile.h"
#include "rgb/rgb_frame.h"
#include "rgb/signals/signals.h"
#include "usb/usb.h"

/* Sends the oldest report of the TX ring if the HID IN endpoint of the protocol interface
 * is free, and removes it from the ring when TinyUSB accepted it. Called from the main loop,
 * from the report-complete callback and before a new report is queued. */
void protocol_flush_tx_ring(void)
{
    uint8_t instance = tud_hid_itf_num_to_instance(USB_ITF_PROTOCOL);

    if (g_protocol_tx_ring.count == 0u) {
        return;
    }

    const protocol_tx_entry_t *entry = &g_protocol_tx_ring.entries[g_protocol_tx_ring.rd];

    if (tud_hid_n_ready(instance)
        && tud_hid_n_report(instance, entry->report_id, entry->data, entry->len)) {
        uint8_t rd = (uint8_t)(g_protocol_tx_ring.rd + 1u);

        if (rd == PROTOCOL_TX_RING_SIZE) {
            rd = 0u;
        }
        g_protocol_tx_ring.rd = rd;
        g_protocol_tx_ring.count--;
    }
}

/* TinyUSB callback: an IN report of a HID interface was sent. For the protocol interface the
 * next report waiting in the TX ring is started. report and len are unused. */
void tud_hid_report_complete_cb(uint8_t instance, uint8_t const *report, uint8_t len)
{
    if (instance == tud_hid_itf_num_to_instance(USB_ITF_PROTOCOL)) {
        protocol_flush_tx_ring();
    }
}

/* Sends one frame (response or event, at most PROTOCOL_FRAME_SIZE bytes) to the host. The
 * frame is zero padded to the smallest of the six IN report sizes that holds it and sent at
 * once if the endpoint is free and nothing is waiting; otherwise it is stored in the TX ring.
 * A frame that arrives while the ring is full is dropped. */
void protocol_queue_report(const uint8_t *data, uint32_t len)
{
    uint8_t  padded[PROTOCOL_FRAME_SIZE];
    uint32_t report_size;
    uint8_t  report_id;

    /* Smallest IN report that holds the frame. */
    if (len <= 32u) {
        report_size = 32u;
        report_id = PROTOCOL_IN_REPORT_32;
    } else if (len <= 62u) {
        report_size = 62u;
        report_id = PROTOCOL_IN_REPORT_62;
    } else if (len <= 254u) {
        report_size = 254u;
        report_id = PROTOCOL_IN_REPORT_254;
    } else if (len <= 510u) {
        report_size = 510u;
        report_id = PROTOCOL_IN_REPORT_510;
    } else if (len <= 1022u) {
        report_size = 1022u;
        report_id = PROTOCOL_IN_REPORT_1022;
    } else {
        report_size = PROTOCOL_FRAME_SIZE;
        report_id = PROTOCOL_IN_REPORT_2046;
    }

    uint8_t instance = tud_hid_itf_num_to_instance(USB_ITF_PROTOCOL);

    if (len < report_size) {
        memcpy(padded, data, len);
        memset(&padded[len], 0, report_size - len);
        data = padded;
        len = report_size;
    }

    if (g_protocol_tx_ring.count != 0u) {
        protocol_flush_tx_ring();
    }

    if (g_protocol_tx_ring.count == 0u && tud_hid_n_ready(instance)) {
        tud_hid_n_report(instance, report_id, data, (uint16_t)len);
    } else if (g_protocol_tx_ring.count < PROTOCOL_TX_RING_SIZE) {
        /* The callers never pass more than PROTOCOL_FRAME_SIZE bytes; len is not checked here. */
        protocol_tx_entry_t *entry = &g_protocol_tx_ring.entries[g_protocol_tx_ring.wr];

        memcpy(entry->data, data, len);
        entry->len = (uint16_t)len;

        uint8_t wr = (uint8_t)(g_protocol_tx_ring.wr + 1u);

        if (wr == PROTOCOL_TX_RING_SIZE) {
            wr = 0u;
        }
        g_protocol_tx_ring.wr = wr;
        entry->report_id = report_id;
        g_protocol_tx_ring.count++;
    }
    /* else: three frames are waiting, the new one is dropped. */
}

/* Sends an unsolicited event to the host: byte 0xEE, 16-bit payload length, then the nanopb
 * encoding of the Event message. Nothing is sent if encoding fails or USB is not mounted.
 * The subscription flags are checked by the callers. */
void protocol_send_event(const Event *event)
{
    protocol_event_frame_t frame;

    memset(&frame, 0, sizeof(frame));
    frame.id = PROTOCOL_EVENT_FRAME_ID;

    pb_ostream_t stream = pb_ostream_from_buffer(frame.payload, PROTOCOL_EVENT_PAYLOAD);

    if (!pb_encode(&stream, &Event_msg, event)) {
        return;
    }
    frame.length = (uint16_t)stream.bytes_written;

    if (g_usb_mounted != 0u) {
        uint32_t frame_len = (uint16_t)(stream.bytes_written + offsetof(protocol_event_frame_t, payload));

        if (frame_len <= PROTOCOL_FRAME_SIZE) {
            protocol_queue_report((const uint8_t *)&frame, frame_len);
        }
    }
}

/* Sends a header-only reply (no payload) with `status` to feature command `cmd`. */
static void protocol_send_status(uint8_t cmd, uint8_t status)
{
    struct {
        uint16_t magic;
        uint8_t  cmd;
        uint8_t  status;
        uint16_t length;
    } reply;

    reply.magic = PROTOCOL_MAGIC;
    reply.cmd = cmd;
    reply.status = status;
    reply.length = 0u;

    if (g_usb_mounted != 0u) {
        protocol_queue_report((const uint8_t *)&reply, PROTOCOL_RESPONSE_HEADER);
    }
}

/* The handler registered for feature command `number`, or NULL. A linear walk over the
 * list: a command arrives at most once per USB transfer and the list has fewer than a
 * hundred entries, so the walk costs a small fraction of the transfer that carried it. */
static protocol_feature_handler_t protocol_feature_handler(uint32_t number)
{
    REGISTRY_FOREACH(protocol_feature_commands, protocol_feature_command_t, command) {
        if (command->number == number) {
            return command->handler;
        }
    }
    return NULL;
}

/* The handler registered for OUT-report command `number`, or NULL; the same linear walk
 * as protocol_feature_handler. */
static protocol_report_handler_t protocol_report_handler(uint32_t number)
{
    REGISTRY_FOREACH(protocol_report_commands, protocol_report_command_t, command) {
        if (command->number == number) {
            return command->handler;
        }
    }
    return NULL;
}

/* Whether feature command `number` is queued: every number from
 * PROTOCOL_FEATURE_CMD_FIRST_CURRENT to the last one (without a handler it is answered
 * 0xAA when it runs), a lower number only if it has a handler. */
static bool protocol_feature_command_accepted(uint32_t number)
{
    if (number >= PROTOCOL_FEATURE_CMD_COUNT) {
        return false;
    }
    return number >= PROTOCOL_FEATURE_CMD_FIRST_CURRENT || protocol_feature_handler(number) != NULL;
}

/* Runs one queued feature command (main loop): calls the handler with the argument word and
 * a zeroed response buffer, then fills in magic and command byte and sends header plus
 * payload. A payload length above PROTOCOL_RESPONSE_PAYLOAD gets status 0x99 and is not
 * sent. A number from PROTOCOL_FEATURE_CMD_FIRST_CURRENT up without a handler is answered
 * with status 0xAA, a lower one with status 0x66 (protocol_receive_feature_report does not
 * queue the latter). */
void protocol_handle_feature_command(const protocol_feature_cmd_t *entry)
{
    protocol_response_t response;

    memset(&response, 0, sizeof(response));

    protocol_feature_handler_t handler = protocol_feature_handler(entry->cmd);

    if (handler != NULL) {
        handler(entry->arg, &response);
    } else if (entry->cmd >= PROTOCOL_FEATURE_CMD_FIRST_CURRENT) {
        response.status = PROTOCOL_STATUS_UNSUPPORTED;
    } else {
        protocol_send_status(entry->cmd, PROTOCOL_STATUS_UNKNOWN_CMD);
        return;
    }

    response.cmd = entry->cmd;

    uint32_t frame_len = (uint16_t)(response.length + PROTOCOL_RESPONSE_HEADER);

    if (response.length > PROTOCOL_RESPONSE_PAYLOAD) {
        response.status = PROTOCOL_STATUS_TOO_LONG;
    }
    response.magic = PROTOCOL_MAGIC;

    if (g_usb_mounted != 0u && frame_len <= PROTOCOL_FRAME_SIZE) {
        protocol_queue_report((const uint8_t *)&response, frame_len);
    }
}

/* Runs the OUT-report command waiting in `report` (main loop): a report with the right
 * magic and a registered number goes to its handler, anything else is ignored. No
 * response. */
void protocol_handle_report_command(const uint8_t *report)
{
    const protocol_report_header_t *header = (const protocol_report_header_t *)report;

    if (header->magic != PROTOCOL_MAGIC) {
        return;
    }

    protocol_report_handler_t handler = protocol_report_handler(header->cmd);

    if (handler != NULL) {
        handler(report);
    }
}

/* Appends one entry (feature command or OUT report) to g_protocol_cmd_queue. The caller
 * holds the critical section and has checked that there is room. */
static void protocol_cmd_queue_push(const protocol_cmd_queue_entry_t *entry)
{
    uint8_t wr = g_protocol_cmd_queue_wr;

    g_protocol_cmd_queue_wr = (uint8_t)(wr + 1u);
    if (g_protocol_cmd_queue_wr == PROTOCOL_CMD_QUEUE_SIZE) {
        g_protocol_cmd_queue_wr = 0u;
    }
    g_protocol_cmd_queue[wr] = *entry;
    g_protocol_cmd_queue_count++;
}

/* SET_REPORT(feature) on the protocol interface: D1 DA cmd arg[4]. An accepted command is
 * queued for the main loop. Answered at once with a header-only reply: status 0xFF for a
 * wrong magic, 0x66 for a number that is not accepted, 0x77 when three feature commands are
 * already waiting. */
static void protocol_receive_feature_report(const uint8_t *buffer)
{
    const protocol_feature_request_t *request = (const protocol_feature_request_t *)buffer;
    uint8_t cmd = request->cmd;

    if (request->magic != PROTOCOL_MAGIC) {
        protocol_send_status(cmd, PROTOCOL_STATUS_ERROR);
        return;
    }
    if (!protocol_feature_command_accepted(cmd)) {
        protocol_send_status(cmd, PROTOCOL_STATUS_UNKNOWN_CMD);
        return;
    }

    uint32_t arg = request->arg;
    uint32_t irq_state = hal_irq_save();

    /* Count the feature commands that are already waiting. */
    uint8_t index = g_protocol_cmd_queue_rd;
    uint8_t pending = 0u;

    for (uint8_t i = 0u; i < g_protocol_cmd_queue_count; i++) {
        if (g_protocol_cmd_queue[index].type == PROTOCOL_CMD_TYPE_FEATURE) {
            pending++;
        }
        index++;
        if (index == PROTOCOL_CMD_QUEUE_SIZE) {
            index = 0u;
        }
    }

    if (pending >= PROTOCOL_FEATURE_CMD_PENDING) {
        hal_irq_restore(irq_state);
        protocol_send_status(cmd, PROTOCOL_STATUS_QUEUE_FULL);
        return;
    }

    /* The padding bytes of the entry are not significant. */
    protocol_cmd_queue_entry_t entry = {
        .type = PROTOCOL_CMD_TYPE_FEATURE,
        .feature = { .cmd = cmd, .arg = arg },
    };

    protocol_cmd_queue_push(&entry);
    hal_irq_restore(irq_state);
}

/* OUT report on the protocol interface (OUT endpoint or SET_REPORT(output)): D1 DA cmd ...
 * A report with a wrong magic or without handler is ignored. The buffer is not copied:
 * g_protocol_rx_report points into TinyUSB's buffer until the main loop has run the handler.
 * Returns false when the caller must not re-arm the OUT endpoint yet. */
static bool protocol_receive_out_report(uint8_t report_id, const uint8_t *buffer, uint16_t bufsize)
{
    uint16_t len = (bufsize <= PROTOCOL_FRAME_SIZE) ? bufsize : (uint16_t)PROTOCOL_FRAME_SIZE;

    if (report_id == 0u) {
        /* Data from the OUT endpoint starts with the report-id byte. */
        if (bufsize == 0u) {
            return true;
        }
        buffer++;
        len--;
    }

    protocol_cmd_queue_entry_t entry;

    memset(&entry, 0, sizeof(entry));

    if (len <= offsetof(protocol_report_plain_t, cmd)) {
        return true;
    }

    const protocol_report_plain_t *report = (const protocol_report_plain_t *)buffer;

    if (report->magic != PROTOCOL_MAGIC) {
        return true;
    }
    if (protocol_report_handler(report->cmd) == NULL) {
        return true;
    }

    uint32_t irq_state = hal_irq_save();
    const uint8_t *busy = g_protocol_rx_report;

    hal_irq_restore(irq_state);
    if (busy != NULL) {
        /* The previous report has not been handled yet; this one stays in the endpoint
         * buffer and is lost when the endpoint is armed again. */
        return false;
    }

    irq_state = hal_irq_save();
    g_protocol_rx_report = buffer;
    hal_irq_restore(irq_state);

    irq_state = hal_irq_save();
    if (g_protocol_cmd_queue_count >= PROTOCOL_CMD_QUEUE_SIZE) {
        hal_irq_restore(irq_state);

        irq_state = hal_irq_save();
        g_protocol_rx_report = NULL;
        hal_irq_restore(irq_state);
        return true;
    }

    entry.type = PROTOCOL_CMD_TYPE_REPORT;
    protocol_cmd_queue_push(&entry);
    hal_irq_restore(irq_state);
    return false;
}

/* Keyboard LED output report. Every lock state that changed is stored in g_usb_link_flags,
 * shown on its indicator key and published as an RGB signal. */
static void protocol_apply_keyboard_leds(uint8_t leds)
{
    uint32_t scroll_lock = (leds & KEYBOARD_LED_SCROLLLOCK) != 0u;

    if (((g_usb_link_flags & USB_LINK_SCROLL_LOCK) != 0u) != scroll_lock) {
        g_usb_link_flags = (uint8_t)((g_usb_link_flags & ~USB_LINK_SCROLL_LOCK)
                                     | (scroll_lock ? USB_LINK_SCROLL_LOCK : 0u));
        rgb_set_lock_indicator(RGB_INDICATOR_SCROLL_LOCK, scroll_lock);
        signals_publish_if_changed(RGB_SIGNAL_SCROLL_LOCK, scroll_lock ? 1.0f : 0.0f);
    }

    uint32_t caps_lock = (leds & KEYBOARD_LED_CAPSLOCK) != 0u;

    if (((g_usb_link_flags & USB_LINK_CAPS_LOCK) != 0u) != caps_lock) {
        g_usb_link_flags = (uint8_t)((g_usb_link_flags & ~USB_LINK_CAPS_LOCK)
                                     | (caps_lock ? USB_LINK_CAPS_LOCK : 0u));
        rgb_set_lock_indicator(RGB_INDICATOR_CAPS_LOCK, caps_lock);
        signals_publish_if_changed(RGB_SIGNAL_CAPS_LOCK, caps_lock ? 1.0f : 0.0f);
    }

    uint32_t num_lock = leds & KEYBOARD_LED_NUMLOCK;

    if (((g_usb_link_flags & USB_LINK_NUM_LOCK) != 0u) != num_lock) {
        g_usb_link_flags = (uint8_t)((g_usb_link_flags & ~USB_LINK_NUM_LOCK)
                                     | (num_lock ? USB_LINK_NUM_LOCK : 0u));

        /* The indicator shows "Num Lock off" unless the global setting is present and false. */
        uint32_t indicator_on;

        if (!g_profile_global_settings.has_num_lock_indicator_inverted
            || g_profile_global_settings.num_lock_indicator_inverted) {
            indicator_on = num_lock ^ 1u;
        } else {
            indicator_on = num_lock;
        }
        rgb_set_lock_indicator(RGB_INDICATOR_NUM_LOCK, indicator_on);
        signals_publish_if_changed(RGB_SIGNAL_NUM_LOCK, num_lock ? 1.0f : 0.0f);
    }
}

/* TinyUSB callback: SET_REPORT or OUT-endpoint data of a HID interface. Protocol interface:
 * feature reports and OUT reports are checked and queued for the main loop. Keyboard
 * interface: the LED output report is applied.
 * The TinyUSB copy in third_party/tinyusb is changed so that this callback returns a value.
 * false = the OUT report stays in the interface buffer and the OUT endpoint must not be
 * armed again yet (the main loop does that after it has run the handler). */
bool tud_hid_set_report_cb(uint8_t instance, uint8_t report_id, hid_report_type_t report_type,
                           uint8_t const *buffer, uint16_t bufsize)
{
    if (instance == tud_hid_itf_num_to_instance(USB_ITF_PROTOCOL)) {
        if (report_type == HID_REPORT_TYPE_OUTPUT) {
            return protocol_receive_out_report(report_id, buffer, bufsize);
        }
        if (report_type == HID_REPORT_TYPE_FEATURE) {
            protocol_receive_feature_report(buffer);
        }
        return true;
    }

    if (instance == tud_hid_itf_num_to_instance(USB_ITF_KEYBOARD)
        && report_type == HID_REPORT_TYPE_OUTPUT && report_id == 0u) {
        protocol_apply_keyboard_leds(buffer[0]);
    }
    return true;
}
