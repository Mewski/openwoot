/*
 * signals.h - public header of feature `signals`: named live values (lock states, layer, profile,
 * key rate, brightness, values set by the host) that the firmware publishes and that lighting
 * elements subscribe to. The host lists the values it may set (feature command 65) and sets them
 * (report command 31). A board lists the feature in board.mk; without it nothing subscribes, a
 * publication does nothing and the feature's host commands, profile sections and start-up step
 * are not registered.
 *
 * Depends on proto/settings.pb.h (SignalRef) and, with the feature, on the public header of
 * feature `lightbar`, whose entries are the subscribers.
 */
#ifndef RGB_SIGNALS_SIGNALS_H
#define RGB_SIGNALS_SIGNALS_H

#include <stdbool.h>
#include <stdint.h>

#include "proto/settings.pb.h"

/* Built-in signals, the `id` of a SignalRef with is_builtin set; their publishers are named. */
enum rgb_signal_id {
    RGB_SIGNAL_ACTIVE_PROFILE = 0,  /* (index + 1) | namespace << 8 (profile activation) */
    RGB_SIGNAL_ANALOG_LEVEL   = 1,  /* deepest key travel of the key pass scaled to 0..255 (keys_process_scan_pass) */
    RGB_SIGNAL_KEY_PRESSED    = 2,  /* 1.0 while a key is pressed, else 0.0 (keys_process_scan_pass) */
    RGB_SIGNAL_CAPS_LOCK      = 3,  /* keyboard LED report of the host */
    RGB_SIGNAL_SCROLL_LOCK    = 4,
    RGB_SIGNAL_NUM_LOCK       = 5,
    RGB_SIGNAL_PROFILE_COUNT  = 6,  /* number of built-in profiles (profile_counts_init, profile commands) */
    RGB_SIGNAL_ACTIVE_LAYER   = 7,  /* layer key actions */
    RGB_SIGNAL_BRIGHTNESS     = 8,  /* key brightness in percent (rgb_step_brightness) */
    RGB_SIGNAL_KEY_RATE       = 9,  /* key presses in the last 990 ms (signals_key_rate_step) */
    RGB_SIGNAL_PROFILE_COLOR  = 10  /* raw 0xFFRRGGBB bits stored in the float (profile activation, report command 26) */
};

#define RGB_SIGNAL_COUNT           11

/*
 * Subscriber id: bits 31:24 name the kind of subscriber, which decides who is notified of a
 * change; the other bits belong to the subscriber. The only kind is a light-bar entry.
 */
#define RGB_SUBSCRIBER_KIND_SHIFT      24
#define RGB_SUBSCRIBER_KIND_LIGHTBAR   2u

#if FEATURE_SIGNALS

/*
 * Registers `subscriber` for a signal and returns the current value of the signal. A host
 * signal is registered on its first subscription and returns 0.0: its value arrives later.
 */
float rgb_signal_subscribe(const SignalRef *signal_ref, uint32_t subscriber);

/* Stores the value of a built-in signal and notifies its subscribers. signal_id is not range
 * checked. */
void rgb_set_signal_value(uint32_t signal_id, float value);

/* rgb_set_signal_value, but only when value differs from the stored one. */
void signals_publish_if_changed(uint32_t signal_id, float value);

/* Drops every subscription and every host signal; the subscribers bind again. */
void signals_clear_subscribers(void);

/* True once the host listed or set the host signals; never false again. */
bool signals_host_values_active(void);

/* Counts a key press into the key-rate window. */
void signals_key_rate_note_press(void);

/* Main-loop step of the key-rate window: every 15 ms the window moves on by one slot and the
 * count of the window is published as RGB_SIGNAL_KEY_RATE. */
void signals_key_rate_step(void);

#else

static inline float rgb_signal_subscribe(const SignalRef *signal_ref, uint32_t subscriber)
{
    return 0.0f;
}

static inline void rgb_set_signal_value(uint32_t signal_id, float value)
{
}

static inline void signals_publish_if_changed(uint32_t signal_id, float value)
{
}

static inline void signals_clear_subscribers(void)
{
}

static inline bool signals_host_values_active(void)
{
    return false;
}

static inline void signals_key_rate_note_press(void)
{
}

static inline void signals_key_rate_step(void)
{
}

#endif /* FEATURE_SIGNALS */

#endif /* RGB_SIGNALS_SIGNALS_H */
