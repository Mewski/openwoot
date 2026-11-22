/*
 * protocol_types.h - types and constants of the `protocol` module (host configuration
 * protocol used by Wootility on HID interface 2).
 *
 * The frame layouts and command numbers match stock firmware 2.14.1; see docs/protocol.md.
 *
 * Transport summary:
 *  - host -> device, feature report (SET_REPORT type 3): D1 DA cmd arg[4]; queued in
 *    g_protocol_cmd_queue, executed by protocol_handle_feature_command, answered with a
 *    protocol_response_t on the IN endpoint.
 *  - host -> device, OUT report (endpoint 0x03 or SET_REPORT type 2): D1 DA cmd ...; the
 *    buffer stays in TinyUSB's epout_buf, g_protocol_rx_report points at it until
 *    protocol_queue_service has run the registered handler of the command
 *    (protocol_handle_report_command). No response.
 *  - device -> host, unsolicited: protocol_event_frame_t (0xEE, length, nanopb Event).
 *  - every IN frame is padded to one of six report sizes (protocol_queue_report).
 *
 * Depends on: nothing (standard headers only).
 */
#ifndef PROTOCOL_TYPES_H
#define PROTOCOL_TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* ---- Framing constants --------------------------------------------------------------------- */

#define PROTOCOL_MAGIC               0xDAD1u  /* little endian bytes D1 DA */
#define PROTOCOL_EVENT_FRAME_ID      0xEEu    /* first byte of an unsolicited frame */
#define PROTOCOL_FRAME_SIZE          2046u    /* 0x7FE: largest IN / OUT frame, size of the response buffer */
#define PROTOCOL_RESPONSE_HEADER     6u       /* magic, cmd, status, length */
#define PROTOCOL_RESPONSE_PAYLOAD    2040u    /* 0x7F8: larger payload lengths get status 0x99 */
#define PROTOCOL_EVENT_PAYLOAD       2043u    /* nanopb stream limit in protocol_send_event */
#define PROTOCOL_FEATURE_CMD_COUNT   89u      /* feature command numbers 0..88; a higher number gets status 0x66 */
/* Lowest feature command number that is answered without a registered handler (status 0xAA);
 * below it a number without handler gets status 0x66. Matches stock firmware 2.14.1 on
 * every board: it answers every number from 38 up, with a handler or with 0xAA. */
#define PROTOCOL_FEATURE_CMD_FIRST_CURRENT 38u
#define PROTOCOL_CMD_QUEUE_SIZE      4u       /* g_protocol_cmd_queue entries */
#define PROTOCOL_FEATURE_CMD_PENDING 3u       /* max queued feature commands; one more gets status 0x77 */
#define PROTOCOL_TX_RING_SIZE        3u       /* g_protocol_tx_ring entries */

/* Response status byte (protocol_response_t.status). */
enum protocol_status {
    PROTOCOL_STATUS_UNKNOWN_CMD = 0x66,  /* number above 88, or below 38 without a handler */
    PROTOCOL_STATUS_QUEUE_FULL  = 0x77,  /* three feature commands already waiting */
    PROTOCOL_STATUS_OK          = 0x88,
    PROTOCOL_STATUS_TOO_LONG    = 0x99,  /* handler reported more than 2040 payload bytes */
    PROTOCOL_STATUS_UNSUPPORTED = 0xAA,  /* number 38..88 without a handler; "no profile" in command 87 */
    PROTOCOL_STATUS_ERROR       = 0xFF,  /* handler error, bad magic */
};

/* IN report ids of interface 2 and their payload sizes (protocol_queue_report):
 * the smallest report that holds the frame is used, the rest is zero padded. */
enum protocol_in_report_id {
    PROTOCOL_IN_REPORT_32   = 1,
    PROTOCOL_IN_REPORT_62   = 2,
    PROTOCOL_IN_REPORT_254  = 3,
    PROTOCOL_IN_REPORT_510  = 4,
    PROTOCOL_IN_REPORT_1022 = 5,
    PROTOCOL_IN_REPORT_2046 = 6,
};

/* ---- Feature commands ------------------------------------------------------------------------ */

/* Feature report as received: D1 DA cmd arg (7 bytes used). */
typedef struct __attribute__((packed)) protocol_feature_request {
    uint16_t magic;  /* +0x00 PROTOCOL_MAGIC */
    uint8_t  cmd;    /* +0x02 enum protocol_feature_cmd_id */
    uint32_t arg;    /* +0x03 little endian, meaning per command (often a profile id in bits 15:0) */
} protocol_feature_request_t;

_Static_assert(sizeof(protocol_feature_request_t) == 7, "protocol_feature_request_t size");
_Static_assert(offsetof(protocol_feature_request_t, arg) == 3, "protocol_feature_request_t.arg");

/* Response frame of a feature command: the 2046-byte buffer protocol_handle_feature_command
 * zeroes on its stack and passes to the handler as second argument. The handler fills
 * status, length and payload; the dispatcher fills magic and cmd afterwards and sends
 * length + 6 bytes. */
typedef struct protocol_response {
    uint16_t magic;                               /* +0x00 PROTOCOL_MAGIC */
    uint8_t  cmd;                                 /* +0x02 echo of the command */
    uint8_t  status;                              /* +0x03 enum protocol_status */
    uint16_t length;                              /* +0x04 payload length */
    uint8_t  payload[PROTOCOL_RESPONSE_PAYLOAD];  /* +0x06 */
} protocol_response_t;

_Static_assert(sizeof(protocol_response_t) == PROTOCOL_FRAME_SIZE, "protocol_response_t size");
_Static_assert(offsetof(protocol_response_t, cmd) == 2, "protocol_response_t.cmd");
_Static_assert(offsetof(protocol_response_t, status) == 3, "protocol_response_t.status");
_Static_assert(offsetof(protocol_response_t, length) == 4, "protocol_response_t.length");
_Static_assert(offsetof(protocol_response_t, payload) == 6, "protocol_response_t.payload");

/* Handler signatures of the feature commands and of the OUT-report commands. */
typedef void (*protocol_feature_handler_t)(uint32_t arg, protocol_response_t *response);
typedef void (*protocol_report_handler_t)(const uint8_t *report);

/* ---- OUT-report commands ---------------------------------------------------------------------- */

/* Common start of every OUT report as seen through g_protocol_rx_report (the optional
 * report-id byte is already skipped by tud_hid_set_report_cb). */
typedef struct protocol_report_header {
    uint16_t magic;  /* +0x00 PROTOCOL_MAGIC */
    uint8_t  cmd;    /* +0x02 enum protocol_report_cmd_id */
} protocol_report_header_t;

_Static_assert(offsetof(protocol_report_header_t, cmd) == 2, "protocol_report_header_t.cmd");

/* protocol_report_profile_t.flags / protocol_report_indexed_t.flags */
#define PROTOCOL_REPORT_FLAG_PERSIST        0x01u  /* bit 0: write to flash (else RAM only) */
#define PROTOCOL_REPORT_FLAG_INDEX_SHIFT    1u     /* bits 5:1 profile index */
#define PROTOCOL_REPORT_FLAG_INDEX_MASK     0x3Eu
#define PROTOCOL_REPORT_FLAG_NS_SHIFT       6u     /* bits 7:6 profile namespace (0 built-in, 1 "lp/") */
#define PROTOCOL_REPORT_FLAG_NS_MASK        0xC0u

/* Layout A: "set" commands 14, 15, 17, 18, 19, 21..26, 29, 32..35, 40: flags at +3, payload
 * length at +4, nanopb payload at +6. Commands 7, 11 and 27 carry fixed-size data from
 * +3 / +4 instead. */
typedef struct __attribute__((packed)) protocol_report_profile {
    uint16_t magic;      /* +0x00 */
    uint8_t  cmd;        /* +0x02 */
    uint8_t  flags;      /* +0x03 PROTOCOL_REPORT_FLAG_* */
    uint16_t length;     /* +0x04 payload length, 0 = clear / delete for some commands */
    uint8_t  payload[];  /* +0x06 */
} protocol_report_profile_t;

_Static_assert(sizeof(protocol_report_profile_t) == 6, "protocol_report_profile_t header size");
_Static_assert(offsetof(protocol_report_profile_t, length) == 4, "protocol_report_profile_t.length");

/* Layout B: commands without a flags byte (12, 28, 30, 31, 39): unaligned length at +3,
 * payload at +5. */
typedef struct __attribute__((packed)) protocol_report_plain {
    uint16_t magic;      /* +0x00 */
    uint8_t  cmd;        /* +0x02 */
    uint16_t length;     /* +0x03 */
    uint8_t  payload[];  /* +0x05 */
} protocol_report_plain_t;

_Static_assert(sizeof(protocol_report_plain_t) == 5, "protocol_report_plain_t header size");
_Static_assert(offsetof(protocol_report_plain_t, length) == 3, "protocol_report_plain_t.length");

/* Layout C: commands 20 (keymap layer) and 44 (al<k> file): flags at +3, layer / slot index
 * at +4, unaligned length at +7, payload at +9. */
typedef struct __attribute__((packed)) protocol_report_indexed {
    uint16_t magic;      /* +0x00 */
    uint8_t  cmd;        /* +0x02 */
    uint8_t  flags;      /* +0x03 PROTOCOL_REPORT_FLAG_* */
    uint8_t  index;      /* +0x04 keymap layer (cmd 20) or al slot 0..15 (cmd 44) */
    uint8_t  unk_05[2];  /* +0x05 not read by either handler */
    uint16_t length;     /* +0x07 */
    uint8_t  payload[];  /* +0x09 */
} protocol_report_indexed_t;

_Static_assert(sizeof(protocol_report_indexed_t) == 9, "protocol_report_indexed_t header size");
_Static_assert(offsetof(protocol_report_indexed_t, length) == 7, "protocol_report_indexed_t.length");

/* ---- Unsolicited events ----------------------------------------------------------------------- */

/* Frame built by protocol_send_event: 0xEE, payload length, nanopb encoding of the 648-byte
 * Event message. */
typedef struct __attribute__((packed)) protocol_event_frame {
    uint8_t  id;                               /* +0x00 PROTOCOL_EVENT_FRAME_ID */
    uint16_t length;                           /* +0x01 encoded payload length */
    uint8_t  payload[PROTOCOL_EVENT_PAYLOAD];  /* +0x03 */
} protocol_event_frame_t;

_Static_assert(sizeof(protocol_event_frame_t) == PROTOCOL_FRAME_SIZE, "protocol_event_frame_t size");
_Static_assert(offsetof(protocol_event_frame_t, payload) == 3, "protocol_event_frame_t.payload");

/* Event kinds = oneof tag of the Event message = index into g_protocol_event_subscriptions.
 * Feature command 76 accepts tags 0, 1, 2 and 4. */
enum protocol_event {
    PROTOCOL_EVENT_NONE              = 0,  /* subscribable, never sent */
    PROTOCOL_EVENT_PROFILE_ACTIVATED = 1,  /* profile_activate: {index, namespace} */
    PROTOCOL_EVENT_LIGHTBAR_CHANGED  = 2,  /* protocol_send_lightbar_changed_event, empty body */
    PROTOCOL_EVENT_TAG3              = 3,  /* 644-byte list message; not subscribable, never sent */
    PROTOCOL_EVENT_PROFILES_CHANGED  = 4,  /* protocol_send_profiles_changed_event, command 44, empty body */
    PROTOCOL_EVENT_COUNT             = 5,
};

/* ---- Command queue (g_protocol_cmd_queue) ----------------------------------------------------- */

enum protocol_cmd_type {
    PROTOCOL_CMD_TYPE_FEATURE = 0,  /* feature command, parameters in the entry */
    PROTOCOL_CMD_TYPE_REPORT  = 1,  /* OUT report waiting in g_protocol_rx_report */
};

/* Queued feature command; protocol_handle_feature_command receives a pointer to this part
 * of the queue entry. */
typedef struct protocol_feature_cmd {
    uint8_t  cmd;         /* +0x00 enum protocol_feature_cmd_id */
    uint8_t  _pad_01[3];
    uint32_t arg;         /* +0x04 the four argument bytes of the feature report */
} protocol_feature_cmd_t;

_Static_assert(sizeof(protocol_feature_cmd_t) == 8, "protocol_feature_cmd_t size");
_Static_assert(offsetof(protocol_feature_cmd_t, arg) == 4, "protocol_feature_cmd_t.arg");

/* One entry of g_protocol_cmd_queue (12 bytes). Written by tud_hid_set_report_cb, consumed
 * by the main loop; both sides mask interrupts. */
typedef struct protocol_cmd_queue_entry {
    uint8_t  type;                   /* +0x00 enum protocol_cmd_type */
    uint8_t  _pad_01[3];
    protocol_feature_cmd_t feature;  /* +0x04 all zero for PROTOCOL_CMD_TYPE_REPORT */
} protocol_cmd_queue_entry_t;

_Static_assert(sizeof(protocol_cmd_queue_entry_t) == 12, "protocol_cmd_queue_entry_t size");
_Static_assert(offsetof(protocol_cmd_queue_entry_t, feature) == 4, "protocol_cmd_queue_entry_t.feature");
_Static_assert(offsetof(protocol_cmd_queue_entry_t, feature.arg) == 8, "protocol_cmd_queue_entry_t.feature.arg");

/* ---- TX ring (g_protocol_tx_ring) ------------------------------------------------------------- */

/* One queued IN report (2050 bytes): frames that could not be handed to the HID IN endpoint
 * at once. Written by protocol_queue_report, sent by protocol_flush_tx_ring. */
typedef struct protocol_tx_entry {
    uint16_t len;                        /* +0x000 padded report size (32 .. 2046) */
    uint8_t  report_id;                  /* +0x002 enum protocol_in_report_id */
    uint8_t  data[PROTOCOL_FRAME_SIZE];  /* +0x003 */
    uint8_t  _pad_801;                   /* +0x801 */
} protocol_tx_entry_t;

_Static_assert(sizeof(protocol_tx_entry_t) == 2050, "protocol_tx_entry_t size");
_Static_assert(offsetof(protocol_tx_entry_t, report_id) == 2, "protocol_tx_entry_t.report_id");
_Static_assert(offsetof(protocol_tx_entry_t, data) == 3, "protocol_tx_entry_t.data");

/* The TX ring object: entries plus read index, write index and fill count (0x180A bytes). */
typedef struct protocol_tx_ring {
    protocol_tx_entry_t entries[PROTOCOL_TX_RING_SIZE];  /* +0x0000 */
    uint8_t rd;                                          /* +0x1806 index of the oldest entry */
    uint8_t wr;                                          /* +0x1807 index of the next free entry */
    uint8_t count;                                       /* +0x1808 used entries, 0..3 */
} protocol_tx_ring_t;

_Static_assert(sizeof(protocol_tx_ring_t) == 0x180A, "protocol_tx_ring_t size");
_Static_assert(offsetof(protocol_tx_ring_t, rd) == 0x1806, "protocol_tx_ring_t.rd");
_Static_assert(offsetof(protocol_tx_ring_t, wr) == 0x1807, "protocol_tx_ring_t.wr");
_Static_assert(offsetof(protocol_tx_ring_t, count) == 0x1808, "protocol_tx_ring_t.count");

/* ---- Command numbers --------------------------------------------------------------------------- */

/* Feature command numbers (the handler registered with PROTOCOL_FEATURE_COMMAND). A number
 * without a handler is answered as PROTOCOL_FEATURE_CMD_FIRST_CURRENT describes. The handler
 * of command X is protocol_cmd_<x> unless noted; argument and reply are described there. */
enum protocol_feature_cmd_id {
    PROTOCOL_FEATURE_CMD_PING                          =  0,  /* echo test */
    PROTOCOL_FEATURE_CMD_GET_VERSION                   =  1,  /* firmware version */
    PROTOCOL_FEATURE_CMD_RESET_TO_BOOTLOADER           =  2,  /* protocol_cmd_enter_update_mode */
    PROTOCOL_FEATURE_CMD_GET_SERIAL                    =  3,  /* serial number and hardware ids */
    PROTOCOL_FEATURE_CMD_RELOAD_PROFILE_LEGACY         =  7,  /* reload the active profile */
    PROTOCOL_FEATURE_CMD_SAVE_LIVE_RGB                 =  8,  /* store live RGB settings and base colours */
    PROTOCOL_FEATURE_CMD_GET_ACTIVE_PROFILE            = 11,  /* id of the active profile */
    PROTOCOL_FEATURE_CMD_GET_DEVICE_CONFIG             = 19,  /* device settings block */
    PROTOCOL_FEATURE_CMD_GET_KEY_SENSOR_VALUES         = 20,  /* per-key sensor values */
    PROTOCOL_FEATURE_CMD_KEYS_OFF                      = 21,  /* stop key reports */
    PROTOCOL_FEATURE_CMD_KEYS_ON                       = 22,  /* resume key reports */
    PROTOCOL_FEATURE_CMD_ACTIVATE_PROFILE              = 23,  /* switch to a profile */
    PROTOCOL_FEATURE_CMD_ACK_EMPTY                     = 24,  /* answers OK, length 0 */
    PROTOCOL_FEATURE_CMD_SOFT_RESET                    = 25,  /* protocol_cmd_restart */
    PROTOCOL_FEATURE_CMD_SDK_SET_KEY_COLOR             = 30,  /* RGB SDK: colour of one key */
    PROTOCOL_FEATURE_CMD_SDK_RESET_KEY_COLOR           = 31,  /* RGB SDK: one key back to its profile colour */
    PROTOCOL_FEATURE_CMD_SDK_RESET_COLORS              = 32,  /* RGB SDK: leave direct mode */
    PROTOCOL_FEATURE_CMD_SDK_INIT_COLORS               = 33,  /* RGB SDK: enter direct mode */
    PROTOCOL_FEATURE_CMD_GET_BASE_COLORS_ROWS_0_2      = 35,  /* base colour map, rows 0..2 */
    PROTOCOL_FEATURE_CMD_GET_BASE_COLORS_ROWS_3_5      = 36,  /* base colour map, rows 3..5 */
    PROTOCOL_FEATURE_CMD_RELOAD_PROFILE                = 38,  /* reload the active profile */
    PROTOCOL_FEATURE_CMD_GET_PROFILE_CFG               = 39,  /* general profile settings (file "cfg") */
    PROTOCOL_FEATURE_CMD_GET_GAMEPAD_MAPPING           = 40,  /* per-key gamepad bindings (file "gm") */
    PROTOCOL_FEATURE_CMD_GET_GAMEPAD_SETTINGS          = 41,  /* stick response settings (file "gp") */
    PROTOCOL_FEATURE_CMD_SAVE_LIVE_CFG                 = 42,  /* store the live general settings */
    PROTOCOL_FEATURE_CMD_FACTORY_RESET                 = 43,  /* erase the file system and reset */
    PROTOCOL_FEATURE_CMD_SET_RAW_SCANNING              = 44,  /* raw sensor values on / off */
    PROTOCOL_FEATURE_CMD_START_XINPUT_DETECTION        = 45,  /* start the gamepad slot detection pattern */
    PROTOCOL_FEATURE_CMD_STOP_XINPUT_DETECTION         = 46,  /* stop the gamepad slot detection pattern */
    PROTOCOL_FEATURE_CMD_ACK                           = 47,  /* answers OK */
    PROTOCOL_FEATURE_CMD_GET_KEYMAP_LAYER              = 48,  /* keymap of one layer (file "ml<k>") */
    PROTOCOL_FEATURE_CMD_GET_ACTUATION_POINTS          = 49,  /* per-key actuation points (file "act") */
    PROTOCOL_FEATURE_CMD_GET_RGB_SETTINGS              = 50,  /* RGB settings (file "rgb") */
    PROTOCOL_FEATURE_CMD_GET_GLOBAL_SETTINGS           = 51,  /* keyboard-wide settings (file "global") */
    PROTOCOL_FEATURE_CMD_GET_ADVANCED_KEYS             = 52,  /* one page of the advanced-key list (file "ak2") */
    PROTOCOL_FEATURE_CMD_SAVE_ADVANCED_KEYS            = 53,  /* store the advanced-key list */
    PROTOCOL_FEATURE_CMD_GET_LEGACY_RAPID_TRIGGER_KEYS = 54,  /* per-key rapid-trigger modes (file "prt") */
    PROTOCOL_FEATURE_CMD_GET_PROFILE_INFO              = 55,  /* profile info record (file "i") */
    PROTOCOL_FEATURE_CMD_FLASH_SELFTEST                = 56,  /* SPI flash and file system self-test */
    PROTOCOL_FEATURE_CMD_GET_LAYER_COLORS              = 57,  /* colour map of one Fn layer (file "r<k>") */
    PROTOCOL_FEATURE_CMD_GET_STORAGE_USAGE             = 58,  /* file-system blocks in use */
    PROTOCOL_FEATURE_CMD_GET_DEVICE_INFO_AUX_RECORD    = 59,  /* auxiliary record of the device info */
    PROTOCOL_FEATURE_CMD_GET_RAPID_TRIGGER_GROUPS      = 61,  /* rapid-trigger groups (file "prt2") */
    PROTOCOL_FEATURE_CMD_GET_PROFILE_COUNT             = 62,  /* number of profiles in a namespace */
    PROTOCOL_FEATURE_CMD_DELETE_PROFILE                = 63,  /* delete the last profile of a namespace */
    PROTOCOL_FEATURE_CMD_CREATE_PROFILE                = 64,  /* create a profile directory */
    PROTOCOL_FEATURE_CMD_GET_HOST_SIGNALS              = 65,  /* host-defined signals in use */
    PROTOCOL_FEATURE_CMD_GET_LIGHTBAR_CONFIG           = 66,  /* light-bar configuration (file "lb") */
    PROTOCOL_FEATURE_CMD_GET_GLOBAL_LIGHTBAR_CONFIG    = 67,  /* keyboard-wide light-bar entries (file "glb") */
    PROTOCOL_FEATURE_CMD_GET_LIGHTBAR_COLORS           = 68,  /* current colours of the light bar */
    PROTOCOL_FEATURE_CMD_GET_USB_SPEED                 = 69,  /* negotiated USB speed */
    PROTOCOL_FEATURE_CMD_GET_LBM_FILE                  = 70,  /* profile file "lbm" */
    PROTOCOL_FEATURE_CMD_GET_GLBM                      = 71,  /* root file "glbm" */
    PROTOCOL_FEATURE_CMD_GET_PROFILE_REVISION          = 72,  /* revision counter of a profile (file "pst") */
    PROTOCOL_FEATURE_CMD_GET_ACTIVE_PROFILE_STATE      = 73,  /* active profile, SDK mode, idle state */
    PROTOCOL_FEATURE_CMD_GET_LED_FRAME                 = 74,  /* current colours of all LEDs */
    PROTOCOL_FEATURE_CMD_UNSUBSCRIBE_ALL_EVENTS        = 75,  /* clear every event subscription */
    PROTOCOL_FEATURE_CMD_SUBSCRIBE_EVENT               = 76,  /* subscribe to / unsubscribe from one event */
    PROTOCOL_FEATURE_CMD_GET_LOCK_MODE                 = 81,  /* lock mode of the device */
    PROTOCOL_FEATURE_CMD_GET_SWITCH_PROFILES           = 82,  /* switch profiles (root file "sw") */
    PROTOCOL_FEATURE_CMD_GET_SWITCH_TYPES              = 83,  /* list of known switch types */
    PROTOCOL_FEATURE_CMD_SELECT_LP_PROFILE             = 84,  /* select a profile of the "lp" namespace */
    PROTOCOL_FEATURE_CMD_GET_LP_PROFILE_SELECTION      = 85,  /* profile selected with command 84 */
    PROTOCOL_FEATURE_CMD_GET_AL_FILE_COUNT             = 86,  /* number of "al<k>" files of a profile */
    PROTOCOL_FEATURE_CMD_GET_PROFILE_AL_FILE           = 87,  /* one "al<k>" file */
};

/* OUT-report command numbers (the handler registered with PROTOCOL_REPORT_COMMAND); a number
 * without a handler is ignored. The payload of each command is described at its handler. */
enum protocol_report_cmd_id {
    PROTOCOL_REPORT_CMD_SET_DEVICE_CONFIG             =  7,  /* device settings block */
    PROTOCOL_REPORT_CMD_NOOP                          =  8,  /* accepted and ignored */
    PROTOCOL_REPORT_CMD_SDK_SET_ALL_COLORS            = 11,  /* RGB SDK: colours of all keys */
    PROTOCOL_REPORT_CMD_SET_SERIAL_NUMBER             = 12,  /* program the serial number */
    PROTOCOL_REPORT_CMD_SET_BASE_COLORS_ROWS0_2       = 14,  /* base colour map, rows 0..2 */
    PROTOCOL_REPORT_CMD_SET_BASE_COLORS_ROWS3_5       = 15,  /* base colour map, rows 3..5 */
    PROTOCOL_REPORT_CMD_SET_PROFILE_CFG               = 17,  /* general profile settings (file "cfg") */
    PROTOCOL_REPORT_CMD_SET_GAMEPAD_MAPPING           = 18,  /* per-key gamepad bindings (file "gm") */
    PROTOCOL_REPORT_CMD_SET_GAMEPAD_SETTINGS          = 19,  /* stick response settings (file "gp") */
    PROTOCOL_REPORT_CMD_SET_KEYMAP_LAYER              = 20,  /* keymap of one layer (file "ml<k>") */
    PROTOCOL_REPORT_CMD_SET_ACTUATION_POINTS          = 21,  /* per-key actuation points (file "act") */
    PROTOCOL_REPORT_CMD_SET_RGB_SETTINGS              = 22,  /* RGB settings (file "rgb") */
    PROTOCOL_REPORT_CMD_SET_GLOBAL_SETTINGS           = 23,  /* keyboard-wide settings (file "global") */
    PROTOCOL_REPORT_CMD_SET_ADVANCED_KEYS             = 24,  /* one page of the advanced-key list (file "ak2") */
    PROTOCOL_REPORT_CMD_SET_LEGACY_RAPID_TRIGGER_KEYS = 25,  /* per-key rapid-trigger modes (file "prt") */
    PROTOCOL_REPORT_CMD_SET_PROFILE_INFO              = 26,  /* profile info record (file "i") */
    PROTOCOL_REPORT_CMD_SET_LAYER_COLORS              = 27,  /* colour map of one Fn layer (file "r<k>") */
    PROTOCOL_REPORT_CMD_SET_DEVICE_INFO_AUX_RECORD    = 28,  /* auxiliary record of the device info */
    PROTOCOL_REPORT_CMD_SET_RAPID_TRIGGER_GROUPS      = 29,  /* rapid-trigger groups (file "prt2") */
    PROTOCOL_REPORT_CMD_SET_LIGHTBAR_COLORS           = 30,  /* protocol_report_set_lightbar_colors: host-driven light bar */
    PROTOCOL_REPORT_CMD_SET_HOST_SIGNALS              = 31,  /* values of host-defined signals */
    PROTOCOL_REPORT_CMD_SET_LIGHTBAR_CONFIG           = 32,  /* light-bar configuration (file "lb") */
    PROTOCOL_REPORT_CMD_SET_GLOBAL_LIGHTBAR_CONFIG    = 33,  /* keyboard-wide light-bar entries (file "glb") */
    PROTOCOL_REPORT_CMD_SAVE_LBM                      = 34,  /* protocol_report_save_lbm: profile file "lbm" */
    PROTOCOL_REPORT_CMD_SAVE_GLBM                     = 35,  /* root file "glbm" */
    PROTOCOL_REPORT_CMD_SET_LOCK_MODE                 = 39,  /* lock the device */
    PROTOCOL_REPORT_CMD_SET_SWITCH_PROFILES           = 40,  /* switch profiles (root file "sw") */
    PROTOCOL_REPORT_CMD_SET_AL_FILE                   = 44,  /* write or delete one "al<k>" file */
};

#endif /* PROTOCOL_TYPES_H */
