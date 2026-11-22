/*
 * hal_led_bus.c - the bus to the LED driver chip on a GD32F4xx (interface: hal/hal_led_bus.h).
 *
 * I2C0 at 400 kHz on the pins of wiring.h (alternate function 4, open drain); the data of a
 * write is sent by DMA0 channel 6, sub-peripheral 1 (I2C0 transmit). The enable line is a
 * push-pull output.
 *
 * Depends on: the GD32F4xx peripheral library, CMSIS (__disable_irq, __enable_irq), the board's
 * wiring.h, TinyUSB (tud_task in the waits).
 */
#include "hal/hal_led_bus.h"

#include "gd32f4xx.h"
#include "tusb.h"

#include "wiring.h"

#define LED_BUS_I2C             I2C0
#define LED_BUS_SPEED_HZ        400000u /* fast mode */
#define LED_BUS_OWN_ADDRESS     0xFFu   /* I2C_SADDR0, never used: the microcontroller is master only */
#define LED_BUS_DMA             DMA0
#define LED_BUS_DMA_CH          DMA_CH6
#define LED_BUS_DMA_FTF_FLAG    DMA_FLAG_ADD(DMA_INTF_FTFIF, LED_BUS_DMA_CH - DMA_CH4) /* INTF1 bit 21 */

/* Parameters of the transmit channel, filled once by hal_led_bus_init; every write sets the
 * memory address and the byte count and programs the channel with them. */
static dma_single_data_parameter_struct s_led_bus_dma;

/* Bus clock: i2c_clock_config computes the timing from the APB1 clock at run time
 * (50 MHz): I2C_CTL1 I2CCLK = 50, I2C_RT = 16, I2C_CKCFG = FAST | DTCY | 5 (400 kHz, duty
 * 16/9). Own address register = 0xFF, I2C mode (SMBEN clear), enabled, ACK on. The DMA
 * parameters: memory (incrementing, 8 bit) -> I2C_DATA (fixed), normal mode, priority ultra
 * high. Matches stock firmware 2.14.1. */
void hal_led_bus_init(void)
{
    rcu_periph_clock_enable(RCU_GPIOA);
    rcu_periph_clock_enable(RCU_GPIOB);
    rcu_periph_clock_enable(RCU_GPIOC);
    rcu_periph_clock_enable(RCU_I2C0);
    rcu_periph_clock_enable(RCU_DMA0);

    gpio_af_set(BOARD_LED_I2C_PORT, GPIO_AF_4, BOARD_LED_I2C_SCL_PIN);
    gpio_af_set(BOARD_LED_I2C_PORT, GPIO_AF_4, BOARD_LED_I2C_SDA_PIN);
    gpio_mode_set(BOARD_LED_I2C_PORT, GPIO_MODE_AF, GPIO_PUPD_NONE, BOARD_LED_I2C_SCL_PIN);
    gpio_output_options_set(BOARD_LED_I2C_PORT, GPIO_OTYPE_OD, GPIO_OSPEED_50MHZ, BOARD_LED_I2C_SCL_PIN);
    gpio_mode_set(BOARD_LED_I2C_PORT, GPIO_MODE_AF, GPIO_PUPD_NONE, BOARD_LED_I2C_SDA_PIN);
    gpio_output_options_set(BOARD_LED_I2C_PORT, GPIO_OTYPE_OD, GPIO_OSPEED_50MHZ, BOARD_LED_I2C_SDA_PIN);
    gpio_mode_set(BOARD_LED_ENABLE_PORT, GPIO_MODE_OUTPUT, GPIO_PUPD_NONE, BOARD_LED_ENABLE_PIN);
    gpio_output_options_set(BOARD_LED_ENABLE_PORT, GPIO_OTYPE_PP, GPIO_OSPEED_50MHZ, BOARD_LED_ENABLE_PIN);

    i2c_clock_config(LED_BUS_I2C, LED_BUS_SPEED_HZ, I2C_DTCY_16_9);
    i2c_mode_addr_config(LED_BUS_I2C, I2C_I2CMODE_ENABLE, I2C_ADDFORMAT_7BITS, LED_BUS_OWN_ADDRESS);
    i2c_enable(LED_BUS_I2C);
    i2c_ack_config(LED_BUS_I2C, I2C_ACK_ENABLE);

    s_led_bus_dma.periph_memory_width = DMA_PERIPH_WIDTH_8BIT;
    s_led_bus_dma.number              = 0;
    s_led_bus_dma.memory_inc          = DMA_MEMORY_INCREASE_ENABLE;
    s_led_bus_dma.periph_inc          = DMA_PERIPH_INCREASE_DISABLE;
    s_led_bus_dma.circular_mode       = DMA_CIRCULAR_MODE_DISABLE;
    s_led_bus_dma.memory0_addr        = 0;
    s_led_bus_dma.periph_addr         = (uint32_t)&I2C_DATA(LED_BUS_I2C);
    s_led_bus_dma.direction           = DMA_MEMORY_TO_PERIPH;
    s_led_bus_dma.priority            = DMA_PRIORITY_ULTRA_HIGH;
}

/* The channel reset is the library's dma_deinit: channel disable, the channel registers, the
 * channel's flags. The count register gets the low 8 bits of `length`, which the interface
 * limits to HAL_LED_BUS_MAX_LENGTH. The waits outside the masked section keep USB alive; there
 * is no timeout and no NACK handling. Matches stock firmware 2.14.1. */
void hal_led_bus_write(uint8_t address, const uint8_t *data, uint32_t length)
{
    dma_deinit(LED_BUS_DMA, LED_BUS_DMA_CH);
    s_led_bus_dma.memory0_addr = (uint32_t)data;
    s_led_bus_dma.number = (uint8_t)length;
    dma_single_data_mode_init(LED_BUS_DMA, LED_BUS_DMA_CH, &s_led_bus_dma);
    dma_channel_subperipheral_select(LED_BUS_DMA, LED_BUS_DMA_CH, DMA_SUBPERI1);

    while ((I2C_STAT1(LED_BUS_I2C) & I2C_STAT1_I2CBSY) != 0) {
        tud_task();
    }

    __disable_irq();
    I2C_CTL0(LED_BUS_I2C) |= I2C_CTL0_START;
    while ((I2C_STAT0(LED_BUS_I2C) & I2C_STAT0_SBSEND) == 0) {
    }
    I2C_DATA(LED_BUS_I2C) = address;
    while ((I2C_STAT0(LED_BUS_I2C) & I2C_STAT0_ADDSEND) == 0) {
    }
    /* ADDSEND is cleared by reading STAT0 and then STAT1. */
    (void)I2C_STAT0(LED_BUS_I2C);
    (void)I2C_STAT1(LED_BUS_I2C);
    I2C_CTL1(LED_BUS_I2C) |= I2C_CTL1_DMAON;
    DMA_CHCTL(LED_BUS_DMA, LED_BUS_DMA_CH) |= DMA_CHXCTL_CHEN;
    __enable_irq();

    while ((DMA_INTF1(LED_BUS_DMA) & LED_BUS_DMA_FTF_FLAG) == 0) {
        tud_task();
    }

    I2C_CTL0(LED_BUS_I2C) |= I2C_CTL0_STOP;
    I2C_CTL1(LED_BUS_I2C) &= ~I2C_CTL1_DMAON;
    DMA_CHCTL(LED_BUS_DMA, LED_BUS_DMA_CH) &= ~DMA_CHXCTL_CHEN;
    while ((I2C_CTL0(LED_BUS_I2C) & I2C_CTL0_STOP) != 0) {
        tud_task();
    }
}

void hal_led_bus_set_enable(bool on)
{
    if (on) {
        gpio_bit_set(BOARD_LED_ENABLE_PORT, BOARD_LED_ENABLE_PIN);
    } else {
        gpio_bit_reset(BOARD_LED_ENABLE_PORT, BOARD_LED_ENABLE_PIN);
    }
}
