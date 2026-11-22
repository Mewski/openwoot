/*
 * hal_led_bus.c - the bus to the LED driver chip on a GD32F30x (interface: hal/hal_led_bus.h).
 *
 * I2C0 at 400 kHz on the pins of wiring.h (alternate function open drain, no remap); the data
 * of a write is sent by DMA0 channel 5 (I2C0 transmit). The enable line is a push-pull
 * output.
 *
 * Depends on: the GD32F30x peripheral library, CMSIS (__disable_irq, __enable_irq), the board's
 * wiring.h, TinyUSB (tud_task in the waits).
 */
#include "hal/hal_led_bus.h"

#include "gd32f30x.h"
#include "tusb.h"

#include "wiring.h"

#define LED_BUS_I2C             I2C0
#define LED_BUS_SPEED_HZ        400000u /* fast mode */
#define LED_BUS_OWN_ADDRESS     0xFFu   /* I2C_SADDR0, never used: the microcontroller is master only */
#define LED_BUS_CLK_MAX_MHZ     60u     /* largest value of the I2CCLK field */
#define LED_BUS_DMA             DMA0
#define LED_BUS_DMA_CH          DMA_CH5
#define LED_BUS_DMA_FTF_FLAG    DMA_FLAG_ADD(DMA_INTF_FTFIF, LED_BUS_DMA_CH) /* INTF bit 21 */

/* Parameters of the transmit channel, filled once by hal_led_bus_init; every write sets the
 * memory address and the byte count and programs the channel with them. */
static dma_parameter_struct s_led_bus_dma;

/* Fast-mode timing from the APB1 clock (60 MHz: I2CCLK = 60, RT = 19, CKCFG = FAST | DTCY | 6),
 * in the statement order of the library's i2c_clock_config with one more statement: the CLKC
 * field is cleared between setting FAST and writing the divider. Matches stock firmware
 * 2.14.1. */
static void led_bus_clock_config(void)
{
    uint32_t pclk1 = rcu_clock_freq_get(CK_APB1);
    uint32_t freq = pclk1 / 1000000u;
    uint32_t clkc;

    if (freq >= LED_BUS_CLK_MAX_MHZ) {
        freq = LED_BUS_CLK_MAX_MHZ;
    }
    I2C_CTL1(LED_BUS_I2C) = (I2C_CTL1(LED_BUS_I2C) & ~I2C_CTL1_I2CCLK) | freq;
    I2C_RT(LED_BUS_I2C) = freq * 300u / 1000u + 1u; /* rise time at most 300 ns */
    I2C_CKCFG(LED_BUS_I2C) |= I2C_CKCFG_DTCY;
    clkc = pclk1 / (LED_BUS_SPEED_HZ * 25u);
    if ((clkc & I2C_CKCFG_CLKC) == 0) {
        clkc |= 1u;
    }
    I2C_CKCFG(LED_BUS_I2C) |= I2C_CKCFG_FAST;
    I2C_CKCFG(LED_BUS_I2C) &= ~I2C_CKCFG_CLKC;
    I2C_CKCFG(LED_BUS_I2C) |= clkc;
}

/* Own address register = 0xFF, I2C mode (SMBEN clear), enabled, ACK on. The DMA parameters:
 * memory (incrementing, 8 bit) -> I2C_DATA (fixed, 8 bit), priority ultra high. Matches stock
 * firmware 2.14.1. */
void hal_led_bus_init(void)
{
    rcu_periph_clock_enable(RCU_GPIOA);
    rcu_periph_clock_enable(RCU_GPIOB);
    rcu_periph_clock_enable(RCU_GPIOC);
    rcu_periph_clock_enable(RCU_I2C0);
    rcu_periph_clock_enable(RCU_DMA0);

    gpio_init(BOARD_LED_I2C_PORT, GPIO_MODE_AF_OD, GPIO_OSPEED_50MHZ, BOARD_LED_I2C_SCL_PIN | BOARD_LED_I2C_SDA_PIN);
    gpio_init(BOARD_LED_ENABLE_PORT, GPIO_MODE_OUT_PP, GPIO_OSPEED_50MHZ, BOARD_LED_ENABLE_PIN);

    led_bus_clock_config();
    i2c_mode_addr_config(LED_BUS_I2C, I2C_I2CMODE_ENABLE, I2C_ADDFORMAT_7BITS, LED_BUS_OWN_ADDRESS);
    i2c_enable(LED_BUS_I2C);
    i2c_ack_config(LED_BUS_I2C, I2C_ACK_ENABLE);

    s_led_bus_dma.periph_width = DMA_PERIPHERAL_WIDTH_8BIT;
    s_led_bus_dma.number       = 0;
    s_led_bus_dma.memory_width = DMA_MEMORY_WIDTH_8BIT;
    s_led_bus_dma.memory_addr  = 0;
    s_led_bus_dma.periph_addr  = (uint32_t)&I2C_DATA(LED_BUS_I2C);
    s_led_bus_dma.priority     = DMA_PRIORITY_ULTRA_HIGH;
    s_led_bus_dma.periph_inc   = DMA_PERIPH_INCREASE_DISABLE;
    s_led_bus_dma.memory_inc   = DMA_MEMORY_INCREASE_ENABLE;
    s_led_bus_dma.direction    = DMA_MEMORY_TO_PERIPHERAL;
}

/* The channel reset is the library's dma_deinit: channel disable, the channel registers, the
 * channel's flags. The count register gets the low 8 bits of `length`, which the interface
 * limits to HAL_LED_BUS_MAX_LENGTH. The waits outside the masked section keep USB alive; there
 * is no timeout and no NACK handling. Matches stock firmware 2.14.1. */
void hal_led_bus_write(uint8_t address, const uint8_t *data, uint32_t length)
{
    dma_deinit(LED_BUS_DMA, LED_BUS_DMA_CH);
    s_led_bus_dma.memory_addr = (uint32_t)data;
    s_led_bus_dma.number = (uint8_t)length;
    dma_init(LED_BUS_DMA, LED_BUS_DMA_CH, &s_led_bus_dma);

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

    while ((DMA_INTF(LED_BUS_DMA) & LED_BUS_DMA_FTF_FLAG) == 0) {
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
