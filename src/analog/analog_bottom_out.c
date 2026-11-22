/*
 * analog_bottom_out.c - per-key learning of the bottom-out (fully pressed) sensor level.
 *
 * Runs in the ADC interrupt for every sample of a pressed key. A fast press ("stroke") that ends
 * in a quiet stretch ("plateau") of 15 ms yields one bottom-out observation; observations move
 * the reference (lowest accepted plateau), an optional upper estimate and finally the published
 * estimate key->bo_estimate, which the scan paths copy into key->bottom_level.
 */
#include <math.h>
#include <string.h>

#include "analog/analog.h"

#define BO_SAMPLE_INTERVAL_US      1000u   /* samples closer than this are ignored */
#define BO_STROKE_TIMEOUT_US       25000u  /* a stroke without plateau is dropped after 25 ms */
#define BO_PLATEAU_HOLD_US         14999u  /* a plateau is accepted once it is older than this */
#define BO_STROKE_START_VELOCITY   6.5f    /* counts/ms, plus bottom_level / 22 */
#define BO_QUIET_VELOCITY          2.0f    /* |velocity| at or below this counts as quiet */
#define BO_QUIET_SAMPLES           3u      /* quiet samples in a row that start a plateau */
#define BO_PLATEAU_MAX_VELOCITY    5.0f    /* |velocity| above this ends the plateau */
#define BO_PLATEAU_MAX_SKEW        2.0f    /* allowed asymmetry of max / min around the mean */
#define BO_SCORE_MAX               2000
#define BO_STABILITY_MAX           3000
#define BO_CANDIDATE_HITS          3u      /* hits that promote the candidate to the upper estimate */
#define BO_CANDIDATE_MISSES        3u      /* misses that remove one hit */
#define BO_MARGIN_KNEE             350u    /* estimates below this get no lower-bound margin */

/* Division that yields 0 for a zero divisor, like the Cortex-M UDIV instruction with DIV_0_TRP
 * clear; the blend in analog_bo_update_estimate relies on it. */
static inline uint32_t analog_bo_udiv(uint32_t numerator, uint32_t denominator)
{
    return denominator != 0u ? numerator / denominator : 0u;
}

/* Forget everything learned so far: tracker cleared, reference and estimate set to "none". */
static void analog_bo_tracker_reset(analog_key_state_t *key)
{
    memset(&key->bo_tracker, 0, sizeof(key->bo_tracker));
    key->bo_tracker.lower_bound = ANALOG_VALUE_NONE;
    key->bo_reference = ANALOG_VALUE_NONE;
    key->bo_estimate = ANALOG_VALUE_NONE;
}

/* Follow a press stroke until it reverses or goes quiet; three quiet samples start a plateau. */
static void analog_bo_follow_stroke(analog_bottom_out_tracker_t *tracker, uint32_t value, uint16_t now_us)
{
    float velocity = tracker->velocity;
    float peak = tracker->stroke_peak_velocity;

    if (velocity < -(peak * 0.16666667f)) {
        tracker->stroke_active = 0u; /* the key is going back up */
        return;
    }
    if (velocity > peak) {
        tracker->stroke_peak_velocity = velocity;
    }
    if (fabsf(velocity) > BO_QUIET_VELOCITY) {
        tracker->quiet_samples = 0u;
        return;
    }
    tracker->quiet_samples++;
    if (tracker->quiet_samples < BO_QUIET_SAMPLES) {
        return;
    }
    tracker->plateau_start_us = now_us;
    tracker->plateau_samples = 0u;
    tracker->plateau_first_value = (uint16_t)value;
    tracker->plateau_mean = (float)value;
    tracker->plateau_max = (uint16_t)value;
    tracker->plateau_min = (uint16_t)value;
    tracker->plateau_active = 1u;
}

/* Add a sample to the running plateau.
 * Returns false when the plateau (and with it the stroke) was dropped. */
static bool analog_bo_extend_plateau(analog_bottom_out_tracker_t *tracker, uint32_t value)
{
    float old_mean = tracker->plateau_mean;
    float mean;
    uint32_t samples;

    if (tracker->plateau_max < value) {
        tracker->plateau_max = (uint16_t)value;
    }
    if (tracker->plateau_min > value) {
        tracker->plateau_min = (uint16_t)value;
    }
    samples = (uint16_t)(tracker->plateau_samples + 1u);
    tracker->plateau_samples = (uint16_t)samples;
    mean = ((float)value + old_mean * (float)(int32_t)(samples - 1u)) / (float)samples;
    tracker->plateau_mean = mean;

    if (fabsf(tracker->velocity) > BO_PLATEAU_MAX_VELOCITY
        || (samples != 0u
            && fabsf(((float)tracker->plateau_max - mean) - (mean - (float)tracker->plateau_min))
                   > BO_PLATEAU_MAX_SKEW)) {
        tracker->stroke_active = 0u;
        tracker->plateau_active = 0u;
        return false;
    }
    return true;
}

/* Publish effective strength, confidence and estimate.
 * unused_strength is the part of the strength that the blend towards the upper estimate did not
 * use; gain scales the final safety deduction of up to 10 counts. */
static void analog_bo_publish(analog_key_state_t *key, uint32_t estimate, uint32_t strength_eff,
                              uint32_t unused_strength, uint32_t gain)
{
    analog_bottom_out_tracker_t *tracker = &key->bo_tracker;
    uint32_t confidence;

    tracker->strength_eff = (uint8_t)strength_eff;
    confidence = (uint16_t)(tracker->score + 127u * unused_strength / 255u);
    if (confidence >= (uint32_t)BO_SCORE_MAX) {
        confidence = (uint32_t)BO_SCORE_MAX;
    }
    tracker->confidence = (uint16_t)confidence;

    if (estimate > 500u) {
        uint32_t scale;

        if (estimate > 1499u) {
            scale = 255u * gain;
        } else {
            scale = gain * (255u * (estimate - 500u) / 1000u);
        }
        estimate = (uint16_t)(estimate - 10u * (scale / 255u) / 255u);
    }
    key->bo_estimate = (uint16_t)estimate;
}

/* The reference gains stability when a plateau lowered it (drop != 0) or confirmed it closely
 * (drop == 0). */
static void analog_bo_update_stability(analog_bottom_out_tracker_t *tracker, uint32_t reference,
                                       uint32_t estimate, uint32_t plateau, uint32_t drop, uint32_t strength)
{
    if (drop != 0u) {
        int32_t gain = (uint16_t)((drop >= 100u ? 100u : drop) + 110u);

        if ((int32_t)tracker->stability > BO_STABILITY_MAX - gain) {
            tracker->stability = BO_STABILITY_MAX;
        } else {
            tracker->stability = (uint16_t)(tracker->stability + gain);
        }
        return;
    }
    if (reference > plateau
        || (int32_t)plateau > (int32_t)(estimate + (uint16_t)(30u * strength / 255u + 25u))) {
        return;
    }
    if (tracker->stability >= BO_STABILITY_MAX - 24) {
        tracker->stability = BO_STABILITY_MAX;
    } else {
        tracker->stability = (uint16_t)(tracker->stability + 25u);
    }
}

/* Plateaus above the current upper estimate are collected as a candidate (base, maximum, hits,
 * misses); a clearly higher or three times confirmed candidate becomes the upper estimate.
 * Returns whether an upper estimate exists afterwards. */
static bool analog_bo_update_upper_candidate(analog_bottom_out_tracker_t *tracker, uint32_t estimate,
                                             uint32_t plateau, uint32_t strength)
{
    uint32_t upper = tracker->has_upper != 0u ? tracker->upper : estimate;
    uint32_t hits = tracker->cand_hits;
    uint32_t cand_base;
    uint32_t cand_max;
    uint32_t promoted;

    if (hits != 0u) {
        cand_base = tracker->cand_base;
        if (plateau < cand_base) {
            /* Below the candidate: a miss if the distance exceeds strength * 29 / 255 of the base. */
            if ((int32_t)(cand_base - plateau) > (int32_t)(strength * (29u * cand_base / 255u) / 255u)) {
                uint8_t misses = (uint8_t)(tracker->cand_misses + 1u);

                tracker->cand_misses = misses;
                if (misses == BO_CANDIDATE_MISSES) {
                    hits = (uint8_t)(hits - 1u);
                    tracker->cand_hits = (uint8_t)hits;
                    tracker->cand_misses = 0u;
                    if (hits == 0u) {
                        tracker->cand_base = 0u;
                        tracker->cand_max = 0u;
                    }
                }
            }
            return tracker->has_upper != 0u;
        }
        cand_max = tracker->cand_max;
    } else {
        if (plateau <= upper) {
            return tracker->has_upper != 0u;
        }
        tracker->cand_base = (uint16_t)plateau;
        tracker->cand_max = 0u;
        cand_base = plateau;
        cand_max = 0u;
    }

    hits = (uint8_t)(hits + 1u);
    tracker->cand_misses = 0u;
    if (plateau > cand_max) {
        cand_max = plateau;
        tracker->cand_max = (uint16_t)plateau;
    }
    /* The more hits, the closer the promoted value gets to the candidate maximum. */
    promoted = (uint16_t)(cand_max - (5u * cand_max / 255u) / (hits * hits));
    tracker->cand_hits = (uint8_t)hits;

    if (upper < promoted) {
        tracker->upper = (uint16_t)promoted;
        tracker->has_upper = 1u;
        if (promoted > cand_base) {
            tracker->cand_base = (uint16_t)cand_max;
            tracker->cand_hits = 1u;
            return true;
        }
    }
    if (hits == BO_CANDIDATE_HITS) {
        tracker->has_upper = 1u;
        tracker->upper = (uint16_t)cand_base;
        if (cand_max > cand_base) {
            tracker->cand_base = (uint16_t)cand_max;
            tracker->cand_hits = 1u;
        } else {
            tracker->cand_base = 0u;
            tracker->cand_max = 0u;
            tracker->cand_hits = 0u;
        }
        return true;
    }
    return tracker->has_upper != 0u;
}

/* Derive lower bound, effective strength and the published estimate from the reference-based
 * estimate, the upper estimate and the stability. */
static void analog_bo_update_estimate(analog_key_state_t *key, uint32_t estimate, uint32_t plateau,
                                      uint32_t strength)
{
    analog_bottom_out_tracker_t *tracker = &key->bo_tracker;
    bool has_upper = analog_bo_update_upper_candidate(tracker, estimate, plateau, strength);
    uint32_t anchor = has_upper ? tracker->upper : estimate;
    uint32_t knee = anchor < BO_MARGIN_KNEE ? BO_MARGIN_KNEE : anchor;
    uint32_t margin = strength * (42u * (uint16_t)(knee - BO_MARGIN_KNEE) / 255u) / 255u;
    uint32_t lower_bound = (uint16_t)(anchor - margin);
    uint32_t base;
    uint32_t fraction;
    uint32_t strength_eff;

    tracker->lower_bound = (uint16_t)lower_bound;

    if (estimate <= lower_bound) {
        if (!has_upper) {
            analog_bo_publish(key, estimate, strength, 0u, strength);
            return;
        }
        base = estimate;
    } else {
        /* The estimate lies above the lower bound: pull it towards the bound, the more the less
         * stable the reference is and the further it sticks out. */
        uint32_t over = (uint16_t)(estimate - anchor + margin);
        uint32_t threshold_low = 127u * margin / 255u;
        uint32_t threshold_high = 229u * margin / 255u;
        uint32_t weight = 0u;

        if (over <= (uint16_t)threshold_low || over < (uint16_t)threshold_high) {
            uint32_t stability = tracker->stability;

            if (over > (uint16_t)threshold_low) {
                stability = (uint16_t)(stability - stability * (over - threshold_low)
                                                       / (threshold_high - threshold_low));
            }
            if (stability > 500u) {
                if (stability >= 1000u) {
                    weight = 255u;
                } else {
                    uint32_t ramp = 255u * (stability - 500u) / 500u;

                    weight = ramp * ramp / 255u;
                }
            }
        }
        base = (uint16_t)(lower_bound + over * weight / 255u);
        if (!has_upper) {
            analog_bo_publish(key, base, strength, 0u, strength);
            return;
        }
    }

    /* Blend between base and the upper estimate decides how much of the strength is in effect. */
    fraction = (uint8_t)analog_bo_udiv(255u * (estimate - base), tracker->upper - base);
    strength_eff = strength * fraction / 255u;
    analog_bo_publish(key, base, (uint8_t)strength_eff, (uint16_t)(strength - strength_eff), strength);
}

/* Evaluate a plateau that has been held for 15 ms: it confirms the reference, becomes the new
 * (lower) reference, or is rejected as too shallow; then the estimate is updated. */
static void analog_bo_accept_plateau(analog_key_state_t *key, uint32_t strength)
{
    analog_bottom_out_tracker_t *tracker = &key->bo_tracker;
    uint32_t reference = key->bo_reference;
    uint32_t plateau = tracker->plateau_min;
    uint32_t estimate;
    uint32_t drop = 0u;
    int32_t bonus;

    tracker->plateau_active = 0u;

    if (reference <= plateau) {
        /* At or above the reference: confirms it. */
        bonus = (uint16_t)(-12 * (int32_t)tracker->strength_eff / 255 + 20);
        if ((int32_t)tracker->score > BO_SCORE_MAX - bonus) {
            tracker->score = BO_SCORE_MAX;
        } else {
            tracker->score = (uint16_t)(tracker->score + bonus);
        }
        if (tracker->max_plateau < plateau) {
            tracker->max_plateau = (uint16_t)plateau;
        }
        estimate = reference;
    } else {
        /* Shallower than the reference. */
        uint32_t strength_eff = tracker->strength_eff;
        uint32_t penalty = (uint16_t)(90u * strength_eff / 255u + 60u);

        if (tracker->score >= penalty) {
            tracker->score = (uint16_t)(tracker->score - penalty);
        } else {
            tracker->score = 0u;
        }
        if (tracker->max_plateau < plateau) {
            tracker->max_plateau = (uint16_t)plateau;
        }

        if (reference == ANALOG_VALUE_NONE) {
            /* First observation: must be in the upper half of the assumed travel. */
            if (plateau < ((uint32_t)key->rest_level + key->bottom_level) >> 1) {
                tracker->stroke_active = 0u;
                return;
            }
        } else {
            uint32_t limit = (uint16_t)(123u * reference / 125u - 6u);

            if (strength != 0u && limit >= tracker->lower_bound) {
                limit = tracker->lower_bound;
            }
            if (plateau < limit) {
                /* Too shallow to be a bottom-out: keep the reference. */
                tracker->stroke_active = 0u;
                if (strength == 0u) {
                    analog_bo_publish(key, reference, 0u, 0u, 0u);
                } else {
                    analog_bo_update_estimate(key, reference, plateau, strength);
                }
                return;
            }
            if (strength != 0u && tracker->has_upper == 0u) {
                tracker->upper = (uint16_t)reference;
                tracker->has_upper = 1u;
            }
        }

        /* The plateau becomes the new reference. */
        drop = (uint16_t)(reference - plateau);
        key->bo_reference = (uint16_t)plateau;
        bonus = (uint16_t)(-60 * (int32_t)strength_eff / 255 + 100);
        if ((int32_t)tracker->score > BO_SCORE_MAX - bonus) {
            tracker->score = BO_SCORE_MAX;
        } else {
            tracker->score = (uint16_t)(tracker->score + bonus);
        }
        estimate = plateau;
    }

    tracker->stroke_active = 0u;
    if (estimate == ANALOG_VALUE_NONE) {
        return;
    }
    if (strength == 0u) {
        analog_bo_publish(key, estimate, 0u, 0u, 0u);
        return;
    }
    if (reference != ANALOG_VALUE_NONE) {
        analog_bo_update_stability(tracker, reference, estimate, plateau, drop, strength);
    }
    analog_bo_update_estimate(key, estimate, plateau, strength);
}

/* Bottom-out learning, called from the scan paths for every sample of a pressed key. value is the
 * filtered sensor value, now_us the low 16 bits of the microsecond count (hal_time_micros). Samples
 * closer than 1000 us are ignored; the others drive the stroke / plateau state machine described
 * in the file header. */
void analog_update_bottom_out_estimate(analog_key_state_t *key, uint32_t value, uint16_t now_us)
{
    analog_bottom_out_tracker_t *tracker = &key->bo_tracker;
    const analog_switch_params_t *params =
        key->has_switch_params != 0u ? &key->switch_params : &g_analog_default_switch_params;
    uint32_t last_value = tracker->last_sample_value;
    uint32_t strength = params->learn_strength;
    uint32_t elapsed_us;
    float velocity;

    if (last_value == 0u || tracker->strength_cfg != strength) {
        /* First sample, or the switch parameters changed: (re)start the tracker. */
        if (last_value != 0u) {
            analog_bo_tracker_reset(key);
        }
        tracker->strength_cfg = (uint8_t)strength;
        tracker->strength_eff = (uint8_t)strength;
        tracker->last_sample_value = (uint16_t)value;
        tracker->last_sample_us = now_us;
        return;
    }

    elapsed_us = (uint16_t)(now_us - tracker->last_sample_us);
    if (elapsed_us <= BO_SAMPLE_INTERVAL_US) {
        return;
    }
    tracker->last_sample_us = now_us;
    tracker->last_sample_value = (uint16_t)value;
    velocity = ((float)value - (float)last_value) / ((float)elapsed_us / 1000.0f); /* counts per ms */
    tracker->velocity = velocity;

    if (tracker->stroke_active == 0u) {
        if (velocity <= (float)(int32_t)(key->bottom_level / 22u) + BO_STROKE_START_VELOCITY) {
            return;
        }
        tracker->stroke_peak_velocity = velocity;
        tracker->quiet_samples = 0u;
        tracker->stroke_active = 1u;
        tracker->plateau_active = 0u;
        tracker->stroke_start_us = now_us;
        analog_bo_follow_stroke(tracker, value, now_us);
        return;
    }

    if (tracker->plateau_active == 0u) {
        if ((uint16_t)(now_us - tracker->stroke_start_us) > BO_STROKE_TIMEOUT_US) {
            tracker->stroke_active = 0u;
            return;
        }
        analog_bo_follow_stroke(tracker, value, now_us);
        return;
    }

    if (!analog_bo_extend_plateau(tracker, value)) {
        return;
    }
    if ((uint16_t)(now_us - tracker->plateau_start_us) > BO_PLATEAU_HOLD_US) {
        analog_bo_accept_plateau(key, strength);
    }
}
