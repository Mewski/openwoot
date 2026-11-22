/*
 * hal_scan.c - the converter side of the sensor scan on a GD32F30x (interface:
 * hal/hal_scan.h; the per-sample functions are in inline/hal_scan_inline.h).
 *
 * Hardware: every multiplexer has an enable line (active low) and an output on its own
 * input of ADC0; the address lines are outputs of one port. ADC1 converts the zero-field
 * reference input of wiring.h at the same instant as ADC0 (regular parallel mode), on
 * every rank. Both converters average BOARD_SCAN_OVERSAMPLE conversions per result.
 * Channel 1 of TIMER1 (1 MHz, single pulse, output compare "PWM1") triggers each
 * conversion of ADC0; DMA0 channel 0 moves the 32-bit result words (ADC0 in the low half,
 * ADC1 in the high half) into the firmware's buffer; the end-of-conversion interrupt
 * (interrupt 18) calls hal_scan_conversion_isr. Single-input scanning converts the one
 * enabled input at rank 0; parallel scanning converts one rank per input, all in one
 * trigger (discontinuous group of BOARD_MUX_COUNT).
 *
 * Depends on: the GD32F30x peripheral library and CMSIS (NVIC_EnableIRQ); the board's
 * board.h and wiring.h (address lines, enable lines, converter inputs, reference input);
 * hal_scan_wait_ms of the firmware.
 */
#include <stddef.h>

#include "hal/hal_scan.h"

#include "gd32f30x.h"

#include "board.h"
#include "wiring.h"

#define SCAN_ADC                ADC0
#define SCAN_REF_ADC            ADC1
#define SCAN_TIMER              TIMER1
#define SCAN_TIMER_PRESCALER    119u /* 120 MHz timer clock / 120 = 1 MHz */
#define SCAN_TIMER_PERIOD       0xFFFFu
#define SCAN_DMA                DMA0
#define SCAN_DMA_CH             DMA_CH0
#define SCAN_ADDRESS_RESET_SHIFT 16u /* the upper half of the bit operate register resets pins */

/* Sample time of every conversion, from the board number in half converter clock cycles. */
#define SCAN_SAMPLE_TIME_NONE   0xFFFFFFFFu
#define SCAN_SAMPLE_TIME                                                    \
    (BOARD_SCAN_SAMPLE_HALF_CYCLES == 3     ? ADC_SAMPLETIME_1POINT5   :    \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 15    ? ADC_SAMPLETIME_7POINT5   :    \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 27    ? ADC_SAMPLETIME_13POINT5  :    \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 57    ? ADC_SAMPLETIME_28POINT5  :    \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 83    ? ADC_SAMPLETIME_41POINT5  :    \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 111   ? ADC_SAMPLETIME_55POINT5  :    \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 143   ? ADC_SAMPLETIME_71POINT5  :    \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 479   ? ADC_SAMPLETIME_239POINT5 :    \
                                              SCAN_SAMPLE_TIME_NONE)

/* Averaging: 2^n conversions per result, shifted right by n so that a result keeps 12 bits. */
#define SCAN_OVERSAMPLE_SHIFT                                               \
    (BOARD_SCAN_OVERSAMPLE == 2   ? 1 : BOARD_SCAN_OVERSAMPLE == 4   ? 2 :  \
     BOARD_SCAN_OVERSAMPLE == 8   ? 3 : BOARD_SCAN_OVERSAMPLE == 16  ? 4 :  \
     BOARD_SCAN_OVERSAMPLE == 32  ? 5 : BOARD_SCAN_OVERSAMPLE == 64  ? 6 :  \
     BOARD_SCAN_OVERSAMPLE == 128 ? 7 : BOARD_SCAN_OVERSAMPLE == 256 ? 8 : 0)

_Static_assert(SCAN_SAMPLE_TIME != SCAN_SAMPLE_TIME_NONE,
               "BOARD_SCAN_SAMPLE_HALF_CYCLES: not a sample time of the GD32F30x converter");
_Static_assert(SCAN_OVERSAMPLE_SHIFT != 0,
               "BOARD_SCAN_OVERSAMPLE: the scan of this family averages 2 to 256 conversions (a power of two)");
_Static_assert(BOARD_MUX_POSITIONS == (1 << BOARD_MUX_ADDR_LINES),
               "BOARD_MUX_POSITIONS: one position per combination of the address lines");

/* The registers and bits of inline/hal_scan_inline.h are those of the peripheral library. */
_Static_assert(&HAL_SCAN_ADC_STAT == &ADC_STAT(SCAN_ADC), "HAL_SCAN_ADC_STAT");
_Static_assert(&HAL_SCAN_ADC_CTL1 == &ADC_CTL1(SCAN_ADC), "HAL_SCAN_ADC_CTL1");
_Static_assert(&HAL_SCAN_TIMER_CTL0 == &TIMER_CTL0(SCAN_TIMER), "HAL_SCAN_TIMER_CTL0");
_Static_assert(&HAL_SCAN_TIMER_CNT == &TIMER_CNT(SCAN_TIMER), "HAL_SCAN_TIMER_CNT");
_Static_assert(&HAL_SCAN_TIMER_CH1CV == &TIMER_CH1CV(SCAN_TIMER), "HAL_SCAN_TIMER_CH1CV");
_Static_assert(HAL_SCAN_ADC_STAT_EOC == ADC_STAT_EOC, "HAL_SCAN_ADC_STAT_EOC");
_Static_assert(HAL_SCAN_ADC_ETSRC == ADC_CTL1_ETSRC, "HAL_SCAN_ADC_ETSRC");
_Static_assert(HAL_SCAN_ADC_ETSRC_TIMER == ADC0_1_EXTTRIG_REGULAR_T1_CH1, "HAL_SCAN_ADC_ETSRC_TIMER");
_Static_assert(HAL_SCAN_TIMER_CEN == TIMER_CTL0_CEN, "HAL_SCAN_TIMER_CEN");

/* The bit operate register of the address lines in the inline header is built from the
 * numeric BOARD_MUX_ADDR_PORT_BASE of wiring.h. */
_Static_assert(&HAL_SCAN_ADDRESS_BOP == &GPIO_BOP(BOARD_MUX_ADDR_PORT),
               "BOARD_MUX_ADDR_PORT_BASE: correct wiring.h; it must be the base address of BOARD_MUX_ADDR_PORT");

/* The wiring of the scan (wiring.h): one entry per address line, or per multiplexer. */
static const uint32_t k_hal_scan_mux_addr_pins[] = BOARD_MUX_ADDR_PINS;
static const uint32_t k_hal_scan_mux_enable_ports[] = BOARD_MUX_ENABLE_PORTS;
static const uint32_t k_hal_scan_mux_enable_pins[] = BOARD_MUX_ENABLE_PINS;
static const uint32_t k_hal_scan_mux_input_ports[] = BOARD_MUX_INPUT_PORTS;
static const uint32_t k_hal_scan_mux_input_pins[] = BOARD_MUX_INPUT_PINS;
static const uint32_t k_hal_scan_mux_input_channels[] = BOARD_MUX_INPUT_CHANNELS;

#define SCAN_WIRING_ENTRIES(table) (sizeof(table) / sizeof((table)[0]))
_Static_assert(SCAN_WIRING_ENTRIES(k_hal_scan_mux_addr_pins) == BOARD_MUX_ADDR_LINES,
               "BOARD_MUX_ADDR_PINS (wiring.h): one pin per address line");
_Static_assert(SCAN_WIRING_ENTRIES(k_hal_scan_mux_enable_ports) == BOARD_MUX_COUNT,
               "BOARD_MUX_ENABLE_PORTS (wiring.h): one port per multiplexer");
_Static_assert(SCAN_WIRING_ENTRIES(k_hal_scan_mux_enable_pins) == BOARD_MUX_COUNT,
               "BOARD_MUX_ENABLE_PINS (wiring.h): one pin per multiplexer");
_Static_assert(SCAN_WIRING_ENTRIES(k_hal_scan_mux_input_ports) == BOARD_MUX_COUNT,
               "BOARD_MUX_INPUT_PORTS (wiring.h): one port per multiplexer");
_Static_assert(SCAN_WIRING_ENTRIES(k_hal_scan_mux_input_pins) == BOARD_MUX_COUNT,
               "BOARD_MUX_INPUT_PINS (wiring.h): one pin per multiplexer");
_Static_assert(SCAN_WIRING_ENTRIES(k_hal_scan_mux_input_channels) == BOARD_MUX_COUNT,
               "BOARD_MUX_INPUT_CHANNELS (wiring.h): one channel per multiplexer");

uint32_t g_hal_scan_address_values[BOARD_MUX_POSITIONS];

/* The firmware's result buffer (hal_scan_init). */
static uint32_t *s_scan_samples;

/* ---- Set-up ------------------------------------------------------------------------------ */

/* Address lines: push-pull outputs, low. Enable lines: push-pull outputs, high (all
 * multiplexers off). */
static void scan_outputs_init(void)
{
    for (uint32_t line = 0u; line < BOARD_MUX_ADDR_LINES; line++) {
        uint32_t pin = k_hal_scan_mux_addr_pins[line];

        gpio_init(BOARD_MUX_ADDR_PORT, GPIO_MODE_OUT_PP, GPIO_OSPEED_50MHZ, pin);
        gpio_bit_reset(BOARD_MUX_ADDR_PORT, pin);
    }

    for (uint32_t input = 0u; input < BOARD_MUX_COUNT; input++) {
        uint32_t port = k_hal_scan_mux_enable_ports[input];
        uint32_t pin = k_hal_scan_mux_enable_pins[input];

        gpio_init(port, GPIO_MODE_OUT_PP, GPIO_OSPEED_50MHZ, pin);
        gpio_bit_set(port, pin);
    }
}

/* The value of the bit operate register for every address: set bits for the lines of the
 * address's one bits, reset bits for the others. */
static void scan_address_values_init(void)
{
    for (uint32_t address = 0u; address < BOARD_MUX_POSITIONS; address++) {
        uint32_t value = 0u;

        for (uint32_t line = 0u; line < BOARD_MUX_ADDR_LINES; line++) {
            uint32_t pin = k_hal_scan_mux_addr_pins[line];

            value |= ((address >> line) & 1u) != 0u ? pin : pin << SCAN_ADDRESS_RESET_SHIFT;
        }
        g_hal_scan_address_values[address] = value;
    }
}

/* TIMER1: clock on, reset pulse, 1 MHz, auto-reload 0xFFFF without shadow, up-counting.
 * Channel 1: output on, active high, mode PWM1 (CHCTL0 bits 14:12 = 7), compare shadow
 * off. Single-pulse mode. The counter is started by whoever triggers a conversion. */
static void scan_trigger_init(void)
{
    timer_parameter_struct timer;
    timer_oc_parameter_struct channel;

    rcu_periph_clock_enable(RCU_TIMER1);
    timer_deinit(SCAN_TIMER);

    timer.prescaler         = SCAN_TIMER_PRESCALER;
    timer.alignedmode       = TIMER_COUNTER_EDGE;
    timer.counterdirection  = TIMER_COUNTER_UP;
    timer.period            = SCAN_TIMER_PERIOD;
    timer.clockdivision     = TIMER_CKDIV_DIV1;
    timer.repetitioncounter = 0;
    timer_init(SCAN_TIMER, &timer);
    timer_auto_reload_shadow_disable(SCAN_TIMER);

    channel.outputstate  = TIMER_CCX_ENABLE;
    channel.outputnstate = TIMER_CCXN_DISABLE;
    channel.ocpolarity   = TIMER_OC_POLARITY_HIGH;
    channel.ocnpolarity  = TIMER_OCN_POLARITY_HIGH;
    channel.ocidlestate  = TIMER_OC_IDLE_STATE_LOW;
    channel.ocnidlestate = TIMER_OCN_IDLE_STATE_LOW;
    timer_channel_output_config(SCAN_TIMER, TIMER_CH_1, &channel);
    timer_channel_output_mode_config(SCAN_TIMER, TIMER_CH_1, TIMER_OC_MODE_PWM1);
    timer_channel_output_shadow_config(SCAN_TIMER, TIMER_CH_1, TIMER_OC_SHADOW_DISABLE);

    timer_single_pulse_mode_config(SCAN_TIMER, TIMER_SP_MODE_SINGLE);
}

/* DMA0 channel 0 moves `count` result words per conversion into the result buffer:
 * channel reset, ADC_RDATA(ADC0) fixed -> memory incrementing, 32 bit, priority high,
 * circular, enabled. The DMA0 clock is on since hal_system_init. */
static void scan_transport_start(uint32_t count)
{
    dma_parameter_struct dma;

    dma_deinit(SCAN_DMA, SCAN_DMA_CH);

    dma.periph_addr  = (uint32_t)&ADC_RDATA(SCAN_ADC);
    dma.periph_width = DMA_PERIPHERAL_WIDTH_32BIT;
    dma.memory_addr  = (uint32_t)s_scan_samples;
    dma.memory_width = DMA_MEMORY_WIDTH_32BIT;
    dma.number       = count;
    dma.priority     = DMA_PRIORITY_HIGH;
    dma.periph_inc   = DMA_PERIPH_INCREASE_DISABLE;
    dma.memory_inc   = DMA_MEMORY_INCREASE_ENABLE;
    dma.direction    = DMA_PERIPHERAL_TO_MEMORY;
    dma_init(SCAN_DMA, SCAN_DMA_CH, &dma);

    dma_circulation_enable(SCAN_DMA, SCAN_DMA_CH);
    dma_channel_enable(SCAN_DMA, SCAN_DMA_CH);
}

/* ADC0 and ADC1: clock CK_APB2 / 4, 12 bit, regular parallel mode, right aligned, regular
 * sequence of length 1; every rank of ADC1 converts the reference input. ADC0 is
 * triggered by the rising edge of TIMER1 channel 1, ADC1 by software (in parallel mode it
 * follows ADC0). Both average BOARD_SCAN_OVERSAMPLE conversions per result. End-of-
 * conversion interrupt of ADC0 (interrupt 18, priority 0); each converter enabled and
 * calibrated after 1 ms; a DMA request of ADC0 after every conversion. The converter
 * inputs and the reference input are analog pins. */
static void scan_converter_init(void)
{
    scan_transport_start(1u);

    for (uint32_t input = 0u; input < BOARD_MUX_COUNT; input++) {
        gpio_init(k_hal_scan_mux_input_ports[input], GPIO_MODE_AIN, GPIO_OSPEED_10MHZ,
                  k_hal_scan_mux_input_pins[input]);
    }
    gpio_init(BOARD_SCAN_REF_PORT, GPIO_MODE_AIN, GPIO_OSPEED_10MHZ, BOARD_SCAN_REF_PIN);

    rcu_periph_clock_enable(RCU_ADC0);
    rcu_periph_clock_enable(RCU_ADC1);
    rcu_adc_clock_config(RCU_CKADC_CKAPB2_DIV4);
    adc_resolution_config(SCAN_ADC, ADC_RESOLUTION_12B);
    adc_resolution_config(SCAN_REF_ADC, ADC_RESOLUTION_12B);
    adc_mode_config(ADC_DAUL_REGULAL_PARALLEL);
    adc_data_alignment_config(SCAN_ADC, ADC_DATAALIGN_RIGHT);
    adc_data_alignment_config(SCAN_REF_ADC, ADC_DATAALIGN_RIGHT);
    adc_channel_length_config(SCAN_ADC, ADC_REGULAR_CHANNEL, 1u);
    adc_channel_length_config(SCAN_REF_ADC, ADC_REGULAR_CHANNEL, 1u);
    for (uint32_t rank = 0u; rank < BOARD_MUX_COUNT; rank++) {
        adc_regular_channel_config(SCAN_REF_ADC, (uint8_t)rank, BOARD_SCAN_REF_CHANNEL, SCAN_SAMPLE_TIME);
    }
    adc_external_trigger_source_config(SCAN_ADC, ADC_REGULAR_CHANNEL, ADC0_1_EXTTRIG_REGULAR_T1_CH1);
    adc_external_trigger_source_config(SCAN_REF_ADC, ADC_REGULAR_CHANNEL, ADC0_1_2_EXTTRIG_REGULAR_NONE);
    adc_external_trigger_config(SCAN_ADC, ADC_REGULAR_CHANNEL, ENABLE);
    adc_external_trigger_config(SCAN_REF_ADC, ADC_REGULAR_CHANNEL, ENABLE);
    adc_oversample_mode_config(SCAN_ADC, ADC_OVERSAMPLING_ALL_CONVERT, OVSAMPCTL_OVSS(SCAN_OVERSAMPLE_SHIFT),
                               OVSAMPCTL_OVSR(SCAN_OVERSAMPLE_SHIFT - 1));
    adc_oversample_mode_enable(SCAN_ADC);
    adc_oversample_mode_config(SCAN_REF_ADC, ADC_OVERSAMPLING_ALL_CONVERT, OVSAMPCTL_OVSS(SCAN_OVERSAMPLE_SHIFT),
                               OVSAMPCTL_OVSR(SCAN_OVERSAMPLE_SHIFT - 1));
    adc_oversample_mode_enable(SCAN_REF_ADC);

    NVIC_EnableIRQ(ADC0_1_IRQn);
    adc_interrupt_enable(SCAN_ADC, ADC_INT_EOC);

    adc_enable(SCAN_ADC);
    hal_scan_wait_ms(1u);
    adc_calibration_enable(SCAN_ADC);
    adc_enable(SCAN_REF_ADC);
    hal_scan_wait_ms(1u);
    adc_calibration_enable(SCAN_REF_ADC);

    adc_dma_mode_enable(SCAN_ADC);
}

/* ---- Scan modes -------------------------------------------------------------------------- */

/* A regular sequence of `length` conversions, all converted per trigger (discontinuous
 * group of `length`). With length 1 the two "set" writes store the value just read. */
static void scan_configure_sequence(uint32_t adc, uint32_t length)
{
    ADC_RSQ0(adc) &= ~ADC_RSQ0_RL;
    ADC_RSQ0(adc) |= RSQ0_RL(length - 1u);
    ADC_CTL0(adc) &= ~(ADC_CTL0_DISRC | ADC_CTL0_DISIC);
    ADC_CTL0(adc) &= ~ADC_CTL0_DISNUM;
    ADC_CTL0(adc) |= CTL0_DISNUM(length - 1u);
    ADC_CTL0(adc) |= ADC_CTL0_DISRC;
}

/* Every enable line high: all multiplexers off. */
static void scan_inputs_all_off(void)
{
    for (uint32_t input = 0u; input < BOARD_MUX_COUNT; input++) {
        GPIO_BOP(k_hal_scan_mux_enable_ports[input]) = k_hal_scan_mux_enable_pins[input];
    }
}

/* Rank n of the sequence converts input `input_order[n]`, and every input is enabled. */
static void scan_inputs_parallel(const uint8_t *input_order)
{
    for (uint32_t rank = 0u; rank < BOARD_MUX_COUNT; rank++) {
        uint32_t input = input_order[rank];

        adc_regular_channel_config(SCAN_ADC, (uint8_t)rank, (uint8_t)k_hal_scan_mux_input_channels[input],
                                   SCAN_SAMPLE_TIME);
        GPIO_BC(k_hal_scan_mux_enable_ports[input]) = k_hal_scan_mux_enable_pins[input];
    }
}

/* The reference converter: a sequence of `length` conversions of the reference input,
 * the same grouping as ADC0. */
static void scan_reference_sequence(uint32_t length)
{
    scan_configure_sequence(SCAN_REF_ADC, length);
    for (uint32_t rank = 0u; rank < length; rank++) {
        adc_regular_channel_config(SCAN_REF_ADC, (uint8_t)rank, BOARD_SCAN_REF_CHANNEL, SCAN_SAMPLE_TIME);
    }
}

/* ---- Interface --------------------------------------------------------------------------- */

void hal_scan_init(uint32_t *samples)
{
    s_scan_samples = samples;

    scan_outputs_init();
    scan_address_values_init();
    scan_trigger_init();
    scan_converter_init();
}

/* Enable line of `previous` high, of `next` low; rank 0 converts the input of `next`. */
void hal_scan_select_input(uint32_t previous, uint32_t next)
{
    GPIO_BOP(k_hal_scan_mux_enable_ports[previous]) = k_hal_scan_mux_enable_pins[previous];
    GPIO_BC(k_hal_scan_mux_enable_ports[next]) = k_hal_scan_mux_enable_pins[next];
    adc_regular_channel_config(SCAN_ADC, 0u, (uint8_t)k_hal_scan_mux_input_channels[next], SCAN_SAMPLE_TIME);
}

void hal_scan_enter_single(uint32_t previous)
{
    scan_inputs_all_off();
    GPIO_BOP(k_hal_scan_mux_enable_ports[previous]) = k_hal_scan_mux_enable_pins[previous];
    GPIO_BC(k_hal_scan_mux_enable_ports[0]) = k_hal_scan_mux_enable_pins[0];
    adc_regular_channel_config(SCAN_ADC, 0u, (uint8_t)k_hal_scan_mux_input_channels[0], SCAN_SAMPLE_TIME);
    hal_scan_select_address(0u);
    scan_configure_sequence(SCAN_ADC, 1u);
    scan_reference_sequence(1u);
    scan_transport_start(1u);
}

void hal_scan_enter_parallel(const uint8_t *input_order, uint32_t address)
{
    scan_inputs_all_off();
    scan_inputs_parallel(input_order);
    hal_scan_select_address(address);
    scan_configure_sequence(SCAN_ADC, BOARD_MUX_COUNT);
    scan_reference_sequence(BOARD_MUX_COUNT);
    scan_transport_start(BOARD_MUX_COUNT);
}

/* ADC1 keeps the sequence length it has (1 after hal_scan_enter_single and at start-up)
 * until the next hal_scan_enter_*. Matches stock firmware 2.14.1. */
void hal_scan_sweep_enter_parallel(const uint8_t *input_order)
{
    DMA_CHCTL(SCAN_DMA, SCAN_DMA_CH) &= ~DMA_CHXCTL_CHEN;

    scan_configure_sequence(SCAN_ADC, BOARD_MUX_COUNT);
    scan_transport_start(BOARD_MUX_COUNT);

    if (input_order != NULL) {
        scan_inputs_parallel(input_order);
    }
}

/* The counter is not reset: the delay counts from the stopwatch start of hal_scan_end_cycle,
 * unless that point has passed (update or compare flag set, or counter at or beyond the
 * new compare value); then the counter is placed one tick before the compare value so
 * that the trigger fires at once. */
void hal_scan_trigger_at(uint16_t ticks)
{
    uint16_t compare = (uint16_t)(ticks - 1u);
    uint32_t counter;
    uint32_t flags;

    timer_disable(SCAN_TIMER);

    counter = TIMER_CNT(SCAN_TIMER);
    flags = TIMER_INTF(SCAN_TIMER);
    if ((flags & (TIMER_INTF_UPIF | TIMER_INTF_CH1IF)) != 0u || compare <= (uint16_t)counter) {
        TIMER_CNT(SCAN_TIMER) = (uint32_t)compare - 1u;
    }
    TIMER_CH1CV(SCAN_TIMER) = compare;

    adc_external_trigger_source_config(SCAN_ADC, ADC_REGULAR_CHANNEL, ADC0_1_EXTTRIG_REGULAR_T1_CH1);

    timer_enable(SCAN_TIMER);
}

/* The trigger source of ADC0 becomes 7 (software, and nothing starts a conversion by
 * software); TIMER1 restarts from 0 with both flags cleared as the stopwatch. */
void hal_scan_end_cycle(uint32_t period_us)
{
    ADC_CTL1(SCAN_ADC) &= ~ADC_CTL1_ETSRC;
    ADC_CTL1(SCAN_ADC) |= ADC0_1_2_EXTTRIG_REGULAR_NONE;
    TIMER_INTF(SCAN_TIMER) = ~TIMER_INTF_CH1IF;
    TIMER_INTF(SCAN_TIMER) = ~TIMER_INTF_UPIF;
    TIMER_CNT(SCAN_TIMER) = 0u;
    TIMER_CH1CV(SCAN_TIMER) = period_us;
    TIMER_CTL0(SCAN_TIMER) |= TIMER_CTL0_CEN;
}
