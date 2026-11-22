/*
 * protocol_data.h - RAM globals of the `protocol` module: event subscriptions, the command
 * queue filled by the USB callbacks and the ring of IN reports waiting to be sent. Each
 * variable is described at its definition in protocol_data.c. All of them are .bss.
 *
 * Depends on: protocol_types.h.
 */
#ifndef PROTOCOL_DATA_H
#define PROTOCOL_DATA_H

#include "protocol/protocol_types.h"

extern uint8_t g_protocol_event_subscriptions[PROTOCOL_EVENT_COUNT];
extern uint8_t g_protocol_notify_pending;
extern protocol_cmd_queue_entry_t g_protocol_cmd_queue[PROTOCOL_CMD_QUEUE_SIZE];
/* volatile: written from the USB callback path and from the main loop, so every access
 * reloads them. */
extern volatile uint8_t g_protocol_cmd_queue_rd;
extern volatile uint8_t g_protocol_cmd_queue_wr;
extern volatile uint8_t g_protocol_cmd_queue_count;
extern const uint8_t *g_protocol_rx_report;
extern protocol_tx_ring_t g_protocol_tx_ring;

#endif /* PROTOCOL_DATA_H */
