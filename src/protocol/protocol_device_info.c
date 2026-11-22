/*
 * protocol_device_info.c - commands that read or write the factory device-info record
 * (last sector of the SPI flash, outside the file system): serial number and hardware
 * ids, the auxiliary record (DeviceInfo tag 2) and the lock mode (tag 4).
 */
#include <string.h>

#include "pb_decode.h"
#include "pb_encode.h"

#include "protocol/protocol.h"
#include "drivers/nor_flash.h"
#include "hal/hal_mcu.h"
#include "profile/profile.h"
#include "storage/storage.h"

/* Selector (arg) of feature command 3. */
enum protocol_serial_query {
    PROTOCOL_SERIAL_QUERY_LEGACY    = 0,  /* fixed 11-byte serial block */
    PROTOCOL_SERIAL_QUERY_MCU_ID    = 1,  /* 96-bit unique id of the MCU */
    PROTOCOL_SERIAL_QUERY_PROTOBUF  = 2,  /* SerialNumber message */
    PROTOCOL_SERIAL_QUERY_FLASH_IDS = 3,  /* SPI flash manufacturer, device id, unique id */
};

#define PROTOCOL_MCU_ID_SIZE           12u
#define PROTOCOL_FLASH_UNIQUE_ID_SIZE  8u
#define PROTOCOL_SERIAL_NUMBER_MAX     0xFFFEu  /* largest serial counter of the legacy block */

/* OUT-report command 12: the four bytes in front of the message that allow overwriting
 * a serial number that is already programmed. */
#define PROTOCOL_SERIAL_OVERRIDE_KEY_SIZE  4u
static const uint8_t k_protocol_serial_override_key[PROTOCOL_SERIAL_OVERRIDE_KEY_SIZE] = { 0x5B, 0x13, 0x04, 0x05 };

#define PROTOCOL_SERIAL_PAYLOAD_MAX    0x7F9u

/* OUT-report command 39: payload = mode byte followed by this magic (bytes 5E 46 45 7A). */
#define PROTOCOL_LOCK_MODE_MAGIC       0x7A45465Eu
#define PROTOCOL_LOCK_MODE_PAYLOAD_MIN 5u

/* Payload of a failed device-info read: the int16 error code. */
static void protocol_respond_error_code(protocol_response_t *response, int result)
{
    int16_t code = (int16_t)result;

    response->status = PROTOCOL_STATUS_ERROR;
    response->length = sizeof(code);
    memcpy(response->payload, &code, sizeof(code));
}

/* Feature command 3 with arg 0, legacy reply: 11 bytes, all zero if the device info
 * cannot be read. */
static void protocol_get_serial_legacy(protocol_response_t *response)
{
    struct __attribute__((packed)) {
        uint16_t f1;      /* SerialNumber.f1 */
        uint8_t  f2;
        uint8_t  f3;
        uint16_t f4;
        uint16_t f5;
        uint16_t number;  /* SerialNumber.f6, clamped */
        uint8_t  stage;
    } block;
    DeviceInfo info;

    memset(&block, 0, sizeof(block));
    memset(&info, 0, sizeof(info));

    if (storage_read_device_info(&info) == STORAGE_OK) {
        const SerialNumber *serial = &info.serial_number;

        block.f1 = serial->f1;
        block.f2 = serial->f2;
        block.f3 = serial->f3;
        block.f4 = serial->f4;
        block.f5 = serial->f5;
        block.number = (serial->f6 >= PROTOCOL_SERIAL_NUMBER_MAX) ? (uint16_t)PROTOCOL_SERIAL_NUMBER_MAX
                                                                  : (uint16_t)serial->f6;
        block.stage = serial->stage;
    }

    memcpy(response->payload, &block, sizeof(block));
    response->length = sizeof(block);
    response->status = PROTOCOL_STATUS_OK;
}

/* Feature command 3, identification data selected by arg: 0 = 11-byte legacy serial block,
 * 1 = 12-byte unique id of the MCU, 2 = SerialNumber message, 3 = SPI flash manufacturer,
 * device id and 8-byte unique id. Any other arg: status 0xFF. */
static void protocol_cmd_get_serial(uint32_t arg, protocol_response_t *response)
{
    switch (arg) {
    case PROTOCOL_SERIAL_QUERY_LEGACY:
        protocol_get_serial_legacy(response);
        break;

    case PROTOCOL_SERIAL_QUERY_MCU_ID: {
        uint32_t unique_id[PROTOCOL_MCU_ID_SIZE / 4u];

        hal_mcu_read_unique_id(unique_id);

        response->status = PROTOCOL_STATUS_OK;
        memcpy(response->payload, unique_id, sizeof(unique_id));
        response->length = sizeof(unique_id);
        break;
    }

    case PROTOCOL_SERIAL_QUERY_PROTOBUF: {
        SerialNumber serial;
        DeviceInfo info;

        memset(&serial, 0, sizeof(serial));
        memset(&info, 0, sizeof(info));

        int result = storage_read_device_info(&info);

        if (result != STORAGE_OK) {
            protocol_respond_error_code(response, result);
            break;
        }
        serial = info.serial_number;

        pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);

        if (pb_encode(&stream, &SerialNumber_msg, &serial)) {
            response->length = (uint16_t)stream.bytes_written;
            response->status = PROTOCOL_STATUS_OK;
        } else {
            response->status = PROTOCOL_STATUS_ERROR;
        }
        break;
    }

    case PROTOCOL_SERIAL_QUERY_FLASH_IDS: {
        uint8_t id[PROTOCOL_FLASH_UNIQUE_ID_SIZE];

        id[0] = 0u;
        id[1] = 0u;
        nor_flash_read_id(id);

        uint8_t manufacturer = id[0];
        uint8_t device_id = id[1];

        nor_flash_read_unique_id(id);

        response->status = PROTOCOL_STATUS_OK;
        response->payload[0] = manufacturer;
        response->payload[1] = device_id;
        response->length = 2u + sizeof(id);
        memcpy(&response->payload[2], id, sizeof(id));
        break;
    }

    default:
        response->status = PROTOCOL_STATUS_ERROR;
        break;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_SERIAL, protocol_cmd_get_serial);

/* OUT-report command 12, programs the serial number. Payload: 4-byte override
 * key, then the SerialNumber message. Without the key the write is only allowed while no
 * serial is programmed. No response. */
static void protocol_cmd_set_serial_number(const uint8_t *report)
{
    const protocol_report_plain_t *request = (const protocol_report_plain_t *)report;
    uint32_t length = request->length;
    DeviceInfo info;

    if (memcmp(request->payload, k_protocol_serial_override_key, PROTOCOL_SERIAL_OVERRIDE_KEY_SIZE) != 0) {
        memset(&info, 0, sizeof(info));

        int result = storage_read_device_info(&info);

        if (result == STORAGE_OK) {
            if (info.serial_number.f1 != 0xFFFFu) {
                return; /* already programmed */
            }
        } else if (result != STORAGE_ERR_BLANK) {
            return;
        }
    }

    /* KNOWN ISSUE: the capped payload length is truncated to 8 bits before the key size is
     * subtracted, so messages are decoded with (length & 0xFF) - 4 bytes; a length whose
     * low byte is below 4 gives a huge stream size. Matches stock firmware 2.14.1. */
    uint8_t capped = (uint8_t)((length <= PROTOCOL_SERIAL_PAYLOAD_MAX) ? length : PROTOCOL_SERIAL_PAYLOAD_MAX);
    SerialNumber serial;
    pb_istream_t stream = pb_istream_from_buffer(&request->payload[PROTOCOL_SERIAL_OVERRIDE_KEY_SIZE],
                                                 (size_t)((int)capped - (int)PROTOCOL_SERIAL_OVERRIDE_KEY_SIZE));

    if (!pb_decode(&stream, &SerialNumber_msg, &serial)) {
        return;
    }

    /* Keep the other fields of the record; the result of this read is not checked. */
    memset(&info, 0, sizeof(info));
    storage_read_device_info(&info);
    info.serial_number = serial;
    storage_write_device_info(&info);
    storage_mirror_serial_to_mcu_flash();
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_SERIAL_NUMBER, protocol_cmd_set_serial_number);

/* OUT-report command 28, stores the auxiliary record (DeviceInfo tag 2).
 * No response. */
static void protocol_cmd_set_device_info_aux_record(const uint8_t *report)
{
    const protocol_report_plain_t *request = (const protocol_report_plain_t *)report;
    DeviceInfoAux aux;
    pb_istream_t stream = pb_istream_from_buffer(request->payload, request->length);

    /* The result of the decoder is not checked. */
    pb_decode(&stream, &DeviceInfoAux_msg, &aux);

    DeviceInfo info;

    memset(&info, 0, sizeof(info));
    info.serial_number.f1 = 0xFFFFu; /* "not programmed" if the sector is blank */

    int result = storage_read_device_info(&info);

    if (result == STORAGE_OK || result == STORAGE_ERR_BLANK) {
        info.has_f2 = true;
        info.f2 = aux;
        storage_write_device_info(&info);
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_DEVICE_INFO_AUX_RECORD, protocol_cmd_set_device_info_aux_record);

/* Feature command 59. Reply: the auxiliary record as DeviceInfoAux message; empty payload
 * if it is not stored; status 0xFF with the int16 error code if the record cannot be read. */
static void protocol_cmd_get_device_info_aux_record(uint32_t arg, protocol_response_t *response)
{
    DeviceInfo info;

    memset(&info, 0, sizeof(info));

    int result = storage_read_device_info(&info);

    if (result != STORAGE_OK && result != STORAGE_ERR_BLANK) {
        protocol_respond_error_code(response, result);
        return;
    }
    if (result == STORAGE_ERR_BLANK || !info.has_f2) {
        response->status = PROTOCOL_STATUS_OK;
        response->length = 0u;
        return;
    }

    DeviceInfoAux aux = info.f2;
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);

    if (pb_encode(&stream, &DeviceInfoAux_msg, &aux)) {
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_DEVICE_INFO_AUX_RECORD, protocol_cmd_get_device_info_aux_record);

/* Feature command 81, returns the lock mode (DeviceInfo tag 4) and refreshes
 * its RAM copy. Reply: one byte, 0 if the record or the field is missing. */
static void protocol_cmd_get_lock_mode(uint32_t arg, protocol_response_t *response)
{
    DeviceInfo info;

    memset(&info, 0, sizeof(info));

    if (storage_read_device_info(&info) == STORAGE_OK && info.has_lock_mode) {
        response->length = 1u;
        g_profile_lock_mode = info.lock_mode;
        response->payload[0] = info.lock_mode;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->length = 1u;
        response->payload[0] = 0u;
        response->status = PROTOCOL_STATUS_OK;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_LOCK_MODE, protocol_cmd_get_lock_mode);

/* OUT-report command 39, sets the lock mode permanently. Payload: mode byte, then the magic
 * 5E 46 45 7A. Only the locking values 1 and 3 are accepted, so a lock cannot be removed
 * with this command. No response. */
static void protocol_cmd_set_lock_mode(const uint8_t *report)
{
    const protocol_report_plain_t *request = (const protocol_report_plain_t *)report;

    if (request->length == 0u) {
        return;
    }

    uint8_t mode = request->payload[0];
    uint32_t magic;

    memcpy(&magic, &request->payload[1], sizeof(magic));

    if (!PROFILE_LOCK_MODE_IS_LOCKED(mode)
        || request->length < PROTOCOL_LOCK_MODE_PAYLOAD_MIN
        || magic != PROTOCOL_LOCK_MODE_MAGIC) {
        return;
    }

    g_profile_lock_mode = mode;

    DeviceInfo info;

    memset(&info, 0, sizeof(info));
    if (storage_read_device_info(&info) == STORAGE_OK) {
        info.has_lock_mode = true;
        info.lock_mode = mode;
        storage_write_device_info(&info);
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_LOCK_MODE, protocol_cmd_set_lock_mode);
