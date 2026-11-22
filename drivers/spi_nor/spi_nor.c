/*
 * spi_nor.c - driver of a serial NOR flash chip with the common command set (W25Q64 /
 * GD25Q64 class), implementing drivers/nor_flash.h.
 *
 * Every command is one transfer of hal_nor_bus.h; the driver chooses opcodes, dummy cycles
 * and the split into pages.
 *
 * Depends on: hal_nor_bus.h, TinyUSB (the device task runs during the busy wait).
 */
#include "tusb.h"

#include "drivers/nor_flash.h"
#include "hal/hal_nor_bus.h"
#include "spi_nor.h"

void nor_flash_init(void)
{
    hal_nor_bus_init();
}

/* Polls the status register; the USB device task runs between polls (a sector erase takes
 * tens of milliseconds). */
void nor_flash_wait_ready(void)
{
    uint8_t status;

    for (;;) {
        hal_nor_bus_transfer_in(SPI_NOR_CMD_READ_STATUS, false, 0u, 0u, &status, 1u);
        if ((status & SPI_NOR_STATUS_BUSY) == 0u) {
            break;
        }
        if (tud_inited()) {
            tud_task();
        }
    }
}

void nor_flash_read(uint32_t addr, uint8_t *buf, uint32_t len)
{
    hal_nor_bus_transfer_in(SPI_NOR_CMD_FAST_READ, true, addr, SPI_NOR_FAST_READ_DUMMY_BITS, buf, len);
}

/*
 * Per page: wait for the chip, write enable, page program. The last program cycle is not
 * awaited. The loop body runs at least once, also for len == 0 (write enable plus an empty
 * page program). Matches stock firmware 2.14.1.
 */
void nor_flash_program(uint32_t addr, const uint8_t *data, uint32_t len)
{
    uint32_t remaining = len;
    uint8_t unused_rx;

    do {
        nor_flash_wait_ready();

        uint16_t chunk = (uint16_t)(SPI_NOR_PAGE_SIZE - (uint8_t)addr);
        if (chunk > remaining) {
            chunk = (uint16_t)remaining;
        }

        hal_nor_bus_transfer_in(SPI_NOR_CMD_WRITE_ENABLE, false, 0u, 0u, &unused_rx, 0u);
        hal_nor_bus_transfer_out(SPI_NOR_CMD_PAGE_PROGRAM, addr, &data[len - remaining], chunk);

        remaining -= chunk;
        addr += chunk;
    } while (remaining != 0u);
}

void nor_flash_erase_sector(uint32_t addr)
{
    uint8_t unused_rx;

    hal_nor_bus_transfer_in(SPI_NOR_CMD_WRITE_ENABLE, false, 0u, 0u, &unused_rx, 0u);
    hal_nor_bus_transfer_in(SPI_NOR_CMD_SECTOR_ERASE, true, addr, 0u, &unused_rx, 0u);
}

void nor_flash_read_id(uint8_t id[2])
{
    hal_nor_bus_transfer_in(SPI_NOR_CMD_READ_ID, true, 0u, 0u, id, 2u);
}

void nor_flash_read_unique_id(uint8_t id[8])
{
    hal_nor_bus_transfer_in(SPI_NOR_CMD_UNIQUE_ID, true, 0u, SPI_NOR_UNIQUE_ID_DUMMY_BITS, id, 8u);
}
