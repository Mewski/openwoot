/*
 * protocol_keys_internal.h - helpers shared by the protocol_keys_*.c files.
 *
 * Depends on: nanopb (encoder), profile/profile.h, protocol_types.h.
 */
#ifndef PROTOCOL_PROTOCOL_KEYS_INTERNAL_H_
#define PROTOCOL_PROTOCOL_KEYS_INTERNAL_H_

#include <stdbool.h>
#include <stdint.h>

#include "pb_encode.h"

#include "profile/profile.h"
#include "protocol/protocol_types.h"

/* Buffer for "<index>/<name>" and "lp/<index>/<name>" paths of storage_format_profile_path. */
#define PROTOCOL_PROFILE_PATH_SIZE  12u

/* Successful reply without payload ("nothing stored"). */
static inline void protocol_reply_empty(protocol_response_t *response)
{
    response->length = 0;
    response->status = PROTOCOL_STATUS_OK;
}

/* Encodes a nanopb message into the response payload and sets length and status. */
static inline void protocol_reply_message(protocol_response_t *response, const pb_msgdesc_t *fields,
                                          const void *message)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);

    if (pb_encode(&stream, fields, message)) {
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}

/* True if the profile id names the active profile. */
static inline bool protocol_profile_is_active(uint16_t profile_id)
{
    return PROFILE_ID_INDEX(profile_id) == g_profile_active_id.index
        && PROFILE_ID_NS(profile_id) == g_profile_active_id.ns;
}

#endif /* PROTOCOL_PROTOCOL_KEYS_INTERNAL_H_ */
