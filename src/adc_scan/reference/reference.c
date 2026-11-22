/*
 * reference.c - state of feature reference: the reference level reported to the host.
 */
#include "adc_scan/reference/reference.h"

uint16_t g_adc_scan_ref_level;

_Static_assert(sizeof(g_adc_scan_ref_level) == 2, "g_adc_scan_ref_level size");
