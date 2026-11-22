/*
 * hal_nor_bus.c - command transfers to the settings flash chip on the GD32F30x family
 * (interface: hal/hal_nor_bus.h).
 *
 * The chip hangs on SPI1, a polled full-duplex master at APB1 / 2 = 30 MHz; data, clock and
 * chip select pins are those of wiring.h. Chip select is driven through the bit-clear and
 * bit-set registers of its port.
 *
 * Depends on: the GD32F30x library, wiring.h of the board, hal_irq.h, TinyUSB (tud_task).
 */
#include "gd32f30x.h"
#include "tusb.h"
#include "wiring.h"

#include "hal/hal_irq.h"
#include "hal/hal_nor_bus.h"

#define NOR_BUS_SPI             SPI1
#define NOR_BUS_DUMMY_BYTE      0xFFu
#define NOR_BUS_DUMMY_BYTES_MAX 4u

/* hal_nor_bus_init configures clock and data out with one call, as the stock firmware does. */
_Static_assert(BOARD_SPI_FLASH_SCK_PORT == BOARD_SPI_FLASH_MOSI_PORT, "SPI clock and data out on one port");

/*
 * Full-duplex transfer of one frame: waits for an empty transmit buffer, writes the data
 * register, waits for a full receive buffer and returns the data register. While it waits
 * it runs the USB device task (which does nothing before the stack is started), also with
 * interrupts masked by the caller.
 */
static uint16_t nor_bus_transfer_byte(uint16_t tx)
{
    while (RESET == spi_i2s_flag_get(NOR_BUS_SPI, SPI_FLAG_TBE)) {
        tud_task();
    }
    spi_i2s_data_transmit(NOR_BUS_SPI, tx);

    while (RESET == spi_i2s_flag_get(NOR_BUS_SPI, SPI_FLAG_RBNE)) {
        tud_task();
    }
    return spi_i2s_data_receive(NOR_BUS_SPI);
}

/* Asserts chip select (line low). */
static inline void nor_bus_select(void)
{
    GPIO_BC(BOARD_SPI_FLASH_CS_PORT) = BOARD_SPI_FLASH_CS_PIN;
}

/* Releases chip select (line high). Three NOPs separate the release from the restore of the
 * interrupt mask. Matches stock firmware 2.14.1. */
static inline void nor_bus_deselect(void)
{
    GPIO_BOP(BOARD_SPI_FLASH_CS_PORT) = BOARD_SPI_FLASH_CS_PIN;
    __NOP();
    __NOP();
    __NOP();
}

/* Sends a 24-bit address, most significant byte first. */
static inline void nor_bus_send_address(uint32_t address)
{
    nor_bus_transfer_byte((uint8_t)(address >> 16));
    nor_bus_transfer_byte((uint8_t)(address >> 8));
    nor_bus_transfer_byte((uint8_t)address);
}

/*
 * Clocks of the pin port, of SPI1 and of the alternate-function block; clock and data out as
 * alternate function push-pull, data in floating, chip select as push-pull output, all
 * 50 MHz. spi_init gives SPI_CTL0 = (SPI_CTL0 & 0x3040) | 0x0304 (master, software chip
 * select, 8 bit, most significant bit first, mode 0, prescaler 2) and clears bit 11 of
 * SPI_I2SCTL. Then chip select high, three NOPs, SPI on. Matches stock firmware 2.14.1.
 */
void hal_nor_bus_init(void)
{
    spi_parameter_struct spi;

    rcu_periph_clock_enable(RCU_GPIOB);
    rcu_periph_clock_enable(RCU_SPI1);
    rcu_periph_clock_enable(RCU_AF);

    gpio_init(BOARD_SPI_FLASH_SCK_PORT, GPIO_MODE_AF_PP, GPIO_OSPEED_50MHZ,
              BOARD_SPI_FLASH_SCK_PIN | BOARD_SPI_FLASH_MOSI_PIN);
    gpio_init(BOARD_SPI_FLASH_MISO_PORT, GPIO_MODE_IN_FLOATING, GPIO_OSPEED_50MHZ, BOARD_SPI_FLASH_MISO_PIN);
    gpio_init(BOARD_SPI_FLASH_CS_PORT, GPIO_MODE_OUT_PP, GPIO_OSPEED_50MHZ, BOARD_SPI_FLASH_CS_PIN);

    spi.device_mode          = SPI_MASTER;
    spi.trans_mode           = SPI_TRANSMODE_FULLDUPLEX;
    spi.frame_size           = SPI_FRAMESIZE_8BIT;
    spi.nss                  = SPI_NSS_SOFT;
    spi.endian               = SPI_ENDIAN_MSB;
    spi.clock_polarity_phase = SPI_CK_PL_LOW_PH_1EDGE;
    spi.prescale             = SPI_PSC_2;
    spi_init(NOR_BUS_SPI, &spi);

    gpio_bit_set(BOARD_SPI_FLASH_CS_PORT, BOARD_SPI_FLASH_CS_PIN);
    __NOP();
    __NOP();
    __NOP();
    spi_enable(NOR_BUS_SPI);
}

/* Dummy cycles go out as 0xFF bytes, one per started 8 bits; the data phase clocks 0x00. */
void hal_nor_bus_transfer_in(uint8_t opcode, bool has_address, uint32_t address, uint8_t dummy_bits, uint8_t *rx,
                             uint32_t length)
{
    uint32_t irq_state = hal_irq_save();
    nor_bus_select();

    nor_bus_transfer_byte(opcode);
    if (has_address) {
        nor_bus_send_address(address);
    }
    for (uint32_t sent = 0u; sent < NOR_BUS_DUMMY_BYTES_MAX && dummy_bits > 8u * sent; sent++) {
        nor_bus_transfer_byte(NOR_BUS_DUMMY_BYTE);
    }
    for (uint32_t i = 0u; i < length; i++) {
        rx[i] = (uint8_t)nor_bus_transfer_byte(0u);
    }

    nor_bus_deselect();
    hal_irq_restore(irq_state);
}

void hal_nor_bus_transfer_out(uint8_t opcode, uint32_t address, const uint8_t *tx, uint32_t length)
{
    uint32_t irq_state = hal_irq_save();
    nor_bus_select();

    nor_bus_transfer_byte(opcode);
    nor_bus_send_address(address);
    for (uint32_t i = 0u; i < length; i++) {
        nor_bus_transfer_byte(tx[i]);
    }

    nor_bus_deselect();
    hal_irq_restore(irq_state);
}
