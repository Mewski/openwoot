/*
 * lightbar_condition.c - condition tree of a light-bar entry.
 *
 * An entry may carry a condition: up to two groups, each of up to two terms, combined with
 * LightbarLogicOp (single / AND / OR) on both levels. A term is either a trigger (true for
 * a time after its signal changed) or a comparison of a signal with another signal or a
 * constant, optionally limited to a hold time. This file binds the terms to their signals
 * and evaluates them once per frame.
 */
#include "rgb/lightbar/lightbar_internal.h"

/* Oneof tags are stored as uint16; only the low byte is tested in several places. */
#define TERM_KIND_LOW_BYTE(term) ((uint8_t)(term)->which_kind)

/*
 * Common tail of rgb_lightbar_eval_condition: a trigger, and a comparison with a hold time,
 * only count as active while the hold timer is running.
 */
static uint32_t lightbar_condition_apply_hold_timer(const LightbarConditionTerm *cond,
                                                    const rgb_lightbar_cond_state_t *cond_state,
                                                    uint32_t result)
{
    bool timed = cond->which_kind == LightbarConditionTerm_trigger_tag
              || (cond->which_kind == LightbarConditionTerm_compare_tag && cond->kind.compare.has_hold_ms);

    if (timed && result != 0) {
        return cond_state->hold_timer_ms != 0;
    }
    return result;
}

/*
 * Evaluates one condition term for this frame; returns non-zero while it is active.
 * elapsed_ms: time since the previous frame, runs the hold timer down.
 */
uint32_t rgb_lightbar_eval_condition(const LightbarConditionTerm *cond, rgb_lightbar_cond_state_t *cond_state, uint32_t elapsed_ms)
{
    uint32_t timer = cond_state->hold_timer_ms;
    uint32_t changed;

    if (timer != 0) {
        cond_state->hold_timer_ms = (timer >= elapsed_ms) ? (uint16_t)(timer - elapsed_ms) : 0;
    }

    changed = cond_state->changed;
    if (changed == 0) {
        /* No new input: keep the last result. */
        return lightbar_condition_apply_hold_timer(cond, cond_state, cond_state->active);
    }

    if (TERM_KIND_LOW_BYTE(cond) == LightbarConditionTerm_trigger_tag) {
        /* The signal changed: (re)start the timer. */
        cond_state->hold_timer_ms = (uint16_t)cond->kind.trigger.duration_ms;
        cond_state->active = 1;
        cond_state->changed = 0;
        return lightbar_condition_apply_hold_timer(cond, cond_state, changed);
    }

    if (TERM_KIND_LOW_BYTE(cond) == LightbarConditionTerm_compare_tag) {
        float lhs = cond_state->lhs;
        float rhs;
        uint32_t result;
        bool valid = true;

        switch (cond->kind.compare.operand.which_value) {
        case LightbarOperand_signal_tag:
            rhs = cond_state->rhs;
            break;
        case LightbarOperand_constant_tag:
            rhs = cond->kind.compare.operand.value.constant;
            break;
        default:
            rhs = 0.0f;
            valid = false;
            break;
        }

        result = 0;
        if (valid) {
            switch (cond->kind.compare.comparator) {
            case LightbarComparator_LIGHTBAR_COMPARATOR_EQUAL:
                result = lhs == rhs;
                break;
            case LightbarComparator_LIGHTBAR_COMPARATOR_NOT_EQUAL:
                result = lhs != rhs;
                break;
            case LightbarComparator_LIGHTBAR_COMPARATOR_GREATER:
                result = rhs < lhs;
                break;
            case LightbarComparator_LIGHTBAR_COMPARATOR_GREATER_OR_EQUAL:
                result = rhs <= lhs;
                break;
            case LightbarComparator_LIGHTBAR_COMPARATOR_LESS:
                result = rhs > lhs;
                break;
            case LightbarComparator_LIGHTBAR_COMPARATOR_LESS_OR_EQUAL:
                result = rhs >= lhs;
                break;
            default:
                valid = false;
                break;
            }
        }

        if (!valid) {
            /* Unknown operand kind or comparator: the term is false. `changed` stays set
             * when the term was already inactive. */
            if (cond_state->active != 0) {
                cond_state->active = 0;
                cond_state->changed = 0;
            }
            return 0;
        }

        if (cond_state->active == result) {
            /* No edge; `changed` stays set, so the comparison is repeated every frame. */
            if (result != 0 && cond_state->hold_timer_ms == 0) {
                return !cond->kind.compare.has_hold_ms;
            }
            return result;
        }

        if (result == 0) {
            cond_state->active = 0;
            cond_state->changed = 0;
            return 0;
        }

        /* false -> true edge */
        if (cond->kind.compare.has_hold_ms) {
            cond_state->hold_timer_ms = (uint16_t)cond->kind.compare.hold_ms;
        }
        cond_state->active = 1;
        cond_state->changed = 0;
        return lightbar_condition_apply_hold_timer(cond, cond_state, 1);
    }

    /* Unknown term kind */
    cond_state->changed = 0;
    return lightbar_condition_apply_hold_timer(cond, cond_state, cond_state->active);
}

/*
 * Evaluates one condition group: AND of all terms (1 for an empty group), OR of all terms
 * (0 for an empty group) or the single term (0 unless there is exactly one).
 */
int rgb_lightbar_eval_condition_group(const LightbarConditionGroup *group, rgb_lightbar_cond_state_t *cond_states, uint32_t elapsed_ms)
{
    int result;

    switch (group->op) {
    case LightbarLogicOp_LIGHTBAR_LOGIC_OP_AND:
        result = 1;
        for (uint32_t i = 0; i < group->terms_count; i++) {
            result &= (int)rgb_lightbar_eval_condition(&group->terms[i], &cond_states[i], elapsed_ms);
        }
        return result;

    case LightbarLogicOp_LIGHTBAR_LOGIC_OP_OR:
        result = 0;
        for (uint32_t i = 0; i < group->terms_count; i++) {
            result = (uint8_t)(rgb_lightbar_eval_condition(&group->terms[i], &cond_states[i], elapsed_ms) | (uint32_t)result);
        }
        return result;

    case LightbarLogicOp_LIGHTBAR_LOGIC_OP_SINGLE:
        if (group->terms_count == 1) {
            return (int)rgb_lightbar_eval_condition(&group->terms[0], cond_states, elapsed_ms);
        }
        return 0;

    default:
        return 0;
    }
}

/*
 * Subscribes the inputs of one compare term: the left signal with id `subscriber`, the
 * right operand (if it is a signal) with `subscriber | RGB_SUBSCRIBER_RHS`. Values of
 * built-in signals are taken over at once. Returns 1 and requests an evaluation when every
 * subscribed operand is a built-in signal, 0 when one is a host signal.
 */
int rgb_lightbar_bind_compare_condition(uint32_t subscriber, const LightbarConditionTerm *condition, rgb_lightbar_cond_state_t *cond_state)
{
    const LightbarCompare *compare = &condition->kind.compare;
    float value = rgb_signal_subscribe(&compare->signal, subscriber);
    bool lhs_is_builtin = compare->signal.is_builtin;
    bool rhs_is_signal = compare->operand.which_value == LightbarOperand_signal_tag;

    if (lhs_is_builtin) {
        cond_state->lhs = value;
    }
    if (rhs_is_signal) {
        value = rgb_signal_subscribe(&compare->operand.value.signal, subscriber | RGB_SUBSCRIBER_RHS);
        if (!compare->operand.value.signal.is_builtin) {
            return 0;
        }
        cond_state->rhs = value;
        if (!lhs_is_builtin) {
            return 0;
        }
    } else if (!lhs_is_builtin) {
        return 0;
    }
    cond_state->changed = 1;
    return 1;
}

/*
 * Subscribes all terms of one condition group. `subscriber` already carries the kind, the
 * table bit, the entry index, the condition class and the group index; the term index is
 * added here (not for a single-term group). Returns 1 when every compare term could be
 * initialised from built-in signals (triggers count as 1), 0 otherwise.
 */
int rgb_lightbar_bind_condition_group(uint32_t subscriber, const LightbarConditionGroup *group, rgb_lightbar_cond_state_t *cond_states)
{
    int all_bound;

    if (group->op == LightbarLogicOp_LIGHTBAR_LOGIC_OP_SINGLE) {
        if (group->terms_count == 1) {
            const LightbarConditionTerm *term = &group->terms[0];

            if (TERM_KIND_LOW_BYTE(term) == LightbarConditionTerm_trigger_tag) {
                rgb_signal_subscribe(&term->kind.trigger.signal, subscriber);
                return 1;
            }
            if (TERM_KIND_LOW_BYTE(term) == LightbarConditionTerm_compare_tag) {
                return rgb_lightbar_bind_compare_condition(subscriber, term, cond_states);
            }
        }
        return 0;
    }

    if (group->op != LightbarLogicOp_LIGHTBAR_LOGIC_OP_AND && group->op != LightbarLogicOp_LIGHTBAR_LOGIC_OP_OR) {
        return 0;
    }

    all_bound = 1;
    for (uint32_t i = 0; i < group->terms_count; i++) {
        const LightbarConditionTerm *term = &group->terms[i];
        uint32_t term_subscriber = subscriber | (i << RGB_SUBSCRIBER_COND_SHIFT);

        if (TERM_KIND_LOW_BYTE(term) == LightbarConditionTerm_trigger_tag) {
            rgb_signal_subscribe(&term->kind.trigger.signal, term_subscriber);
        } else if (TERM_KIND_LOW_BYTE(term) == LightbarConditionTerm_compare_tag) {
            all_bound &= rgb_lightbar_bind_compare_condition(term_subscriber, term, &cond_states[i]);
        } else {
            all_bound = 0;
        }
    }
    return all_bound;
}

/*
 * A trigger / compare term passes when its (left) signal is a built-in signal of
 * RGB_SIGNAL_FAST_MODE_MASK; terms of another kind pass. Only the low byte of the signal id
 * is looked at.
 */
static int lightbar_term_allowed_in_fast_mode(const LightbarConditionTerm *term)
{
    uint32_t id;

    if ((uint8_t)(term->which_kind - 1) > 1) {
        return 1;
    }
    /* trigger.signal and compare.signal share the offset */
    if (!term->kind.trigger.signal.is_builtin) {
        return 0;
    }
    id = (uint8_t)term->kind.trigger.signal.id;
    if (id > RGB_SIGNAL_COUNT - 1) {
        return 0;
    }
    return ((1u << id) & RGB_SIGNAL_FAST_MODE_MASK) != 0;
}

/*
 * Filter of rgb_lightbar_init_item_state while the scan mode level is non-zero: the group
 * may only depend on the built-in signals of RGB_SIGNAL_FAST_MODE_MASK (no host signals,
 * no analog level, key pressed or key rate). Pure function.
 */
int rgb_lightbar_group_allowed_in_fast_mode(const LightbarConditionGroup *group)
{
    if (group->op == LightbarLogicOp_LIGHTBAR_LOGIC_OP_SINGLE) {
        if (group->terms_count != 1) {
            return 0;
        }
        return lightbar_term_allowed_in_fast_mode(&group->terms[0]);
    }

    if (group->op != LightbarLogicOp_LIGHTBAR_LOGIC_OP_AND && group->op != LightbarLogicOp_LIGHTBAR_LOGIC_OP_OR) {
        return 0;
    }

    for (uint32_t i = 0; i < group->terms_count; i++) {
        if (!lightbar_term_allowed_in_fast_mode(&group->terms[i])) {
            return 0;
        }
    }
    return 1;
}
