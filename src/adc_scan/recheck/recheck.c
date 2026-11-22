/*
 * recheck.c - state of feature recheck: which conversion of a position is in flight and which
 * readings of a parallel conversion wait for the second one.
 */
#include "adc_scan/recheck/recheck.h"

uint8_t g_adc_scan_recheck_pending;
uint8_t g_adc_scan_recheck_pass;
uint8_t g_adc_scan_recheck_mask;

_Static_assert(sizeof(g_adc_scan_recheck_pending) == 1, "g_adc_scan_recheck_pending size");
_Static_assert(sizeof(g_adc_scan_recheck_pass) == 1, "g_adc_scan_recheck_pass size");
_Static_assert(sizeof(g_adc_scan_recheck_mask) == 1, "g_adc_scan_recheck_mask size");
