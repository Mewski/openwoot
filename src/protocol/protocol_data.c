/*
 * protocol_data.c - definitions of the RAM globals of the `protocol` module. All of them
 * are .bss (no initializers).
 */
#include "protocol/protocol_data.h"

/* Host subscription flag per event kind (enum protocol_event = oneof tag of the Event
 * message): 1 = send that event. Set by feature command 76, cleared by command 75. */
uint8_t g_protocol_event_subscriptions[PROTOCOL_EVENT_COUNT];

/* Set by protocol_delete_last_profile; the main loop then sends the "profiles changed"
 * event (protocol_send_profiles_changed_event) as soon as the TX ring has room. */
uint8_t g_protocol_notify_pending;

/* Commands received by tud_hid_set_report_cb, executed by the main loop. Ring buffer with
 * read index, write index and fill count below; every access runs with interrupts masked. */
protocol_cmd_queue_entry_t g_protocol_cmd_queue[PROTOCOL_CMD_QUEUE_SIZE];
volatile uint8_t g_protocol_cmd_queue_rd;
volatile uint8_t g_protocol_cmd_queue_wr;
volatile uint8_t g_protocol_cmd_queue_count;

/* OUT report being processed (points into TinyUSB's epout_buf of interface 2, after the
 * report-id byte); NULL = none. While it is set the OUT endpoint is not re-armed. */
const uint8_t *g_protocol_rx_report;

/* IN reports waiting for the HID IN endpoint of interface 2: three entries plus read index,
 * write index and fill count. */
protocol_tx_ring_t g_protocol_tx_ring;
