/*
 * analog_rebound.c - release-rebound filter: after a key came back to rest from a real press, a
 * small upward bounce of the sensor value is reported as rebound so that the scan paths do not
 * treat it as a new press.
 */
#include "analog/analog.h"

#define ANALOG_REBOUND_PRESS_PERCENT    30u  /* travel above this arms the filter */
#define ANALOG_REBOUND_BOUNCE_PERCENT   10u  /* travel above this while bouncing is a real press */

enum analog_rebound_trend {
    ANALOG_REBOUND_STABLE,   /* reading within +/-1 count of the previous one */
    ANALOG_REBOUND_RISING,
    ANALOG_REBOUND_FALLING,
};

/* One step of the release-rebound state machine of a key (enum analog_rebound_state). reading is
 * the current sensor value, rest_level and bottom_level bound the travel, now_us is the low 16 bits
 * of the microsecond count (hal_time_micros). Returns 1 while the reading counts as a bounce after
 * a release (the caller reports travel 0), else 0. */
int analog_detect_release_rebound(analog_rebound_filter_t *filter, uint32_t rest_level, uint32_t bottom_level,
                                  uint32_t reading, uint16_t now_us)
{
    uint32_t prev_reading = filter->prev_reading;
    uint32_t clamped = rest_level < reading ? reading : rest_level;
    uint32_t depth = clamped > bottom_level ? bottom_level - rest_level : clamped - rest_level;
    uint32_t range = bottom_level - rest_level;
    /* A zero range yields 0 %: bottom_level == rest threshold is reachable (the scan paths copy
     * bo_estimate unchecked). */
    uint32_t travel_percent = range != 0u ? (uint16_t)(100u * depth / range) : 0u;
    enum analog_rebound_trend trend;
    int timer_running;

    if (prev_reading - reading + 1u <= 2u) {
        trend = ANALOG_REBOUND_STABLE;
    } else if (prev_reading < reading) {
        trend = ANALOG_REBOUND_RISING;
    } else {
        trend = ANALOG_REBOUND_FALLING;
    }
    filter->prev_reading = (uint16_t)reading;

    switch (filter->state) {
    case ANALOG_REBOUND_IDLE:
        if (travel_percent > ANALOG_REBOUND_PRESS_PERCENT) {
            filter->state = ANALOG_REBOUND_PRESSED;
        }
        return 0;

    case ANALOG_REBOUND_PRESSED:
        if (trend == ANALOG_REBOUND_FALLING) {
            filter->state = ANALOG_REBOUND_RELEASING;
        }
        return 0;

    case ANALOG_REBOUND_RELEASING:
        if (travel_percent == 0u) {
            filter->state = ANALOG_REBOUND_RELEASED;
        } else if (trend == ANALOG_REBOUND_RISING) {
            filter->state = ANALOG_REBOUND_IDLE; /* pressed again before reaching rest */
        }
        return 0;

    case ANALOG_REBOUND_RELEASED:
        if (trend == ANALOG_REBOUND_RISING) {
            filter->rest_timer_running = 0u;
            filter->state = ANALOG_REBOUND_REBOUND;
            return 1;
        }
        if (rest_level < reading) {
            if (filter->rest_timer_running != 0u) {
                filter->rest_timer_running = 0u;
            }
        } else if (filter->rest_timer_running == 0u) {
            filter->rest_timer_running = 1u;
            filter->rest_timer_start_us = now_us;
        } else if ((uint16_t)(now_us - filter->rest_timer_start_us) > ANALOG_REBOUND_REST_US - 1u) {
            filter->rest_timer_running = 0u;
            filter->state = ANALOG_REBOUND_IDLE;
        }
        return 0;

    case ANALOG_REBOUND_REBOUND:
        if (travel_percent > ANALOG_REBOUND_BOUNCE_PERCENT) {
            filter->state = ANALOG_REBOUND_IDLE;
            filter->rest_timer_running = 0u;
            return 0;
        }
        timer_running = filter->rest_timer_running;
        if (rest_level < reading) {
            if (timer_running == 0) {
                return 1;
            }
            filter->rest_timer_running = 0u;
            return timer_running; /* the old flag value, i.e. 1 */
        }
        if (timer_running == 0) {
            filter->rest_timer_running = 1u;
            filter->rest_timer_start_us = now_us;
            return 1;
        }
        if ((uint16_t)(now_us - filter->rest_timer_start_us) > ANALOG_REBOUND_REST_US - 1u) {
            filter->rest_timer_running = 0u;
            filter->state = ANALOG_REBOUND_IDLE;
            return 0;
        }
        return timer_running; /* still bouncing: 1 */

    default:
        return 0;
    }
}
