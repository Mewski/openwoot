/*
 * hal_scan.c - the converter side of the sensor scan on a GD32F4xx (interface:
 * hal/hal_scan.h; the per-sample functions are in inline/hal_scan_inline.h).
 *
 * Hardware: every multiplexer has an enable line (active low) and an output on its own
 * input of ADC0; the address lines are outputs of one port. Channel 1 of TIMER1 (1 MHz,
 * single pulse, output compare "PWM1") triggers each conversion of ADC0; DMA1 channel 0
 * (sub-peripheral 0) moves the 32-bit result words into the firmware's buffer; the
 * end-of-conversion interrupt (interrupt 18) calls hal_scan_conversion_isr. Single-input
 * scanning converts the one enabled input at rank 0; parallel scanning converts one
 * rank per input, all in one trigger (discontinuous group of BOARD_MUX_COUNT).
 *
 * Depends on: the GD32F4xx peripheral library and CMSIS (NVIC_EnableIRQ); the board's
 * board.h and wiring.h (address lines, enable lines, converter inputs); hal_scan_wait_ms of
 * the firmware.
 */
#include <stddef.h>

#include "hal/hal_scan.h"

#include "gd32f4xx.h"

#include "board.h"
#include "wiring.h"

#define SCAN_ADC                ADC0
#define SCAN_TIMER              TIMER1
#define SCAN_TIMER_PRESCALER    199u /* 200 MHz timer clock / 200 = 1 MHz */
#define SCAN_TIMER_PERIOD       0xFFFFu
#define SCAN_DMA                DMA1
#define SCAN_DMA_CH             DMA_CH0
#define SCAN_ADDRESS_RESET_SHIFT 16u /* the upper half of the bit operate register resets pins */

/* Sample time of every conversion, from the board number in half converter clock cycles. */
#define SCAN_SAMPLE_TIME_NONE   0xFFFFFFFFu
#define SCAN_SAMPLE_TIME                                                    \
    (BOARD_SCAN_SAMPLE_HALF_CYCLES == 6     ? ADC_SAMPLETIME_3   :          \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 30    ? ADC_SAMPLETIME_15  :          \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 56    ? ADC_SAMPLETIME_28  :          \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 112   ? ADC_SAMPLETIME_56  :          \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 168   ? ADC_SAMPLETIME_84  :          \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 224   ? ADC_SAMPLETIME_112 :          \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 288   ? ADC_SAMPLETIME_144 :          \
     BOARD_SCAN_SAMPLE_HALF_CYCLES == 960   ? ADC_SAMPLETIME_480 :          \
                                              SCAN_SAMPLE_TIME_NONE)

_Static_assert(SCAN_SAMPLE_TIME != SCAN_SAMPLE_TIME_NONE,
               "BOARD_SCAN_SAMPLE_HALF_CYCLES: not a sample time of the GD32F4xx converter");
_Static_assert(BOARD_SCAN_OVERSAMPLE == 1,
               "BOARD_SCAN_OVERSAMPLE: the scan of this family converts every sample once");
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
_Static_assert(HAL_SCAN_ADC_ETSRC_TIMER == ADC_EXTTRIG_REGULAR_T1_CH1, "HAL_SCAN_ADC_ETSRC_TIMER");
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

        gpio_mode_set(BOARD_MUX_ADDR_PORT, GPIO_MODE_OUTPUT, GPIO_PUPD_NONE, pin);
        gpio_output_options_set(BOARD_MUX_ADDR_PORT, GPIO_OTYPE_PP, GPIO_OSPEED_50MHZ, pin);
        gpio_bit_reset(BOARD_MUX_ADDR_PORT, pin);
    }

    for (uint32_t input = 0u; input < BOARD_MUX_COUNT; input++) {
        uint32_t port = k_hal_scan_mux_enable_ports[input];
        uint32_t pin = k_hal_scan_mux_enable_pins[input];

        gpio_mode_set(port, GPIO_MODE_OUTPUT, GPIO_PUPD_NONE, pin);
        gpio_output_options_set(port, GPIO_OTYPE_PP, GPIO_OSPEED_50MHZ, pin);
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

/* TIMER1: clock on, all timers at CK_AHB (RCU_CFG1 bit 24), reset pulse, 1 MHz, auto-reload
 * 0xFFFF without shadow, up-counting. Channel 1: output on, active high, mode PWM1
 * (CHCTL0 bits 14:12 = 7), compare shadow off. Single-pulse mode. The counter is started
 * by whoever triggers a conversion. */
static void scan_trigger_init(void)
{
    timer_parameter_struct timer;
    timer_oc_parameter_struct channel;

    rcu_periph_clock_enable(RCU_TIMER1);
    rcu_timer_clock_prescaler_config(RCU_TIMER_PSC_MUL4);
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

/* DMA1 channel 0 moves `count` result words per conversion into the result buffer:
 * clock on (RCU_AHB1EN bit 22), channel reset, ADC_RDATA(ADC0) fixed -> memory
 * incrementing, 32 bit, priority high, sub-peripheral 0, circular, enabled. */
static void scan_transport_start(uint32_t count)
{
    dma_single_data_parameter_struct dma;

    rcu_periph_clock_enable(RCU_DMA1);
    dma_deinit(SCAN_DMA, SCAN_DMA_CH);

    dma.periph_addr         = (uint32_t)&ADC_RDATA(SCAN_ADC);
    dma.periph_inc          = DMA_PERIPH_INCREASE_DISABLE;
    dma.memory0_addr        = (uint32_t)s_scan_samples;
    dma.memory_inc          = DMA_MEMORY_INCREASE_ENABLE;
    dma.periph_memory_width = DMA_PERIPH_WIDTH_32BIT;
    dma.circular_mode       = DMA_CIRCULAR_MODE_DISABLE;
    dma.direction           = DMA_PERIPH_TO_MEMORY;
    dma.number              = count;
    dma.priority            = DMA_PRIORITY_HIGH;
    dma_single_data_mode_init(SCAN_DMA, SCAN_DMA_CH, &dma);

    dma_channel_subperipheral_select(SCAN_DMA, SCAN_DMA_CH, DMA_SUBPERI0);
    dma_circulation_enable(SCAN_DMA, SCAN_DMA_CH);
    dma_channel_enable(SCAN_DMA, SCAN_DMA_CH);
}

/* ADC0: clock CK_AHB / 5 = 40 MHz, 12 bit, independent mode, right aligned, routine
 * sequence of length 1 triggered by the rising edge of TIMER1 channel 1, end-of-conversion
 * interrupt (interrupt 18, priority 0), enabled and calibrated after 1 ms, a DMA request
 * after every conversion. The converter inputs are analog pins. */
static void scan_converter_init(void)
{
    scan_transport_start(1u);

    for (uint32_t input = 0u; input < BOARD_MUX_COUNT; input++) {
        gpio_mode_set(k_hal_scan_mux_input_ports[input], GPIO_MODE_ANALOG, GPIO_PUPD_NONE,
                      k_hal_scan_mux_input_pins[input]);
    }

    rcu_periph_clock_enable(RCU_ADC0);
    adc_clock_config(ADC_ADCCK_HCLK_DIV5);
    adc_resolution_config(SCAN_ADC, ADC_RESOLUTION_12B);
    adc_sync_mode_config(ADC_SYNC_MODE_INDEPENDENT);
    adc_data_alignment_config(SCAN_ADC, ADC_DATAALIGN_RIGHT);
    adc_channel_length_config(SCAN_ADC, ADC_REGULAR_CHANNEL, 1u);
    adc_external_trigger_source_config(SCAN_ADC, ADC_REGULAR_CHANNEL, ADC_EXTTRIG_REGULAR_T1_CH1);
    adc_external_trigger_config(SCAN_ADC, ADC_REGULAR_CHANNEL, EXTERNAL_TRIGGER_RISING);

    NVIC_EnableIRQ(ADC_IRQn);
    adc_interrupt_enable(SCAN_ADC, ADC_INT_EOC);

    adc_enable(SCAN_ADC);
    hal_scan_wait_ms(1u);
    adc_calibration_enable(SCAN_ADC);

    adc_dma_request_after_last_enable(SCAN_ADC);
    adc_dma_mode_enable(SCAN_ADC);
}

/* ---- Scan modes -------------------------------------------------------------------------- */

/* A routine sequence of `length` conversions, all converted per trigger (discontinuous
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
    scan_transport_start(1u);
}

void hal_scan_enter_parallel(const uint8_t *input_order, uint32_t address)
{
    scan_inputs_all_off();
    scan_inputs_parallel(input_order);
    hal_scan_select_address(address);
    scan_configure_sequence(SCAN_ADC, BOARD_MUX_COUNT);
    scan_transport_start(BOARD_MUX_COUNT);
}

void hal_scan_sweep_enter_parallel(const uint8_t *input_order)
{
    /* KNOWN ISSUE: this disables channel 0 of DMA0, but the results use channel 0 of DMA1.
     * It has no effect, because scan_transport_start re-initialises the right channel
     * below. Matches stock firmware 2.14.1. */
    DMA_CHCTL(DMA0, DMA_CH0) &= ~DMA_CHXCTL_CHEN;

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

    adc_external_trigger_source_config(SCAN_ADC, ADC_REGULAR_CHANNEL, ADC_EXTTRIG_REGULAR_T1_CH1);

    timer_enable(SCAN_TIMER);
}

/* The trigger source of ADC0 becomes 0 (channel 0 of TIMER0, which never runs: the register
 * is read and written back); TIMER1 restarts from 0 with both flags cleared as the
 * stopwatch. */
void hal_scan_end_cycle(uint32_t period_us)
{
    ADC_CTL1(SCAN_ADC) &= ~ADC_CTL1_ETSRC;
    ADC_CTL1(SCAN_ADC) |= ADC_EXTTRIG_REGULAR_T0_CH0;
    TIMER_INTF(SCAN_TIMER) = ~TIMER_INTF_CH1IF;
    TIMER_INTF(SCAN_TIMER) = ~TIMER_INTF_UPIF;
    TIMER_CNT(SCAN_TIMER) = 0u;
    TIMER_CH1CV(SCAN_TIMER) = period_us;
    TIMER_CTL0(SCAN_TIMER) |= TIMER_CTL0_CEN;
}
