/*
 * hal_nor_bus.h - command transfers to the settings flash chip.
 *
 * The bus is driven one command at a time: chip select, opcode, an optional 24-bit address
 * (most significant byte first), dummy cycles, then a data phase in one direction, chip
 * deselect. What the opcodes mean is the business of the flash driver (drivers/spi_nor/);
 * this interface only moves the bytes. A controller that is programmed per command
 * (quad-SPI, OCTOSPI) implements it as well as a plain SPI master.
 *
 * While a transfer waits for the bus, the USB device task keeps running.
 *
 * Depends on: nothing.
 */
#ifndef HAL_NOR_BUS_H
#define HAL_NOR_BUS_H

#include <stdbool.h>
#include <stdint.h>

/* Sets up the bus and the chip select line; the chip is left deselected. */
void hal_nor_bus_init(void);

/* One command that reads: `opcode`, the 24-bit `address` if `has_address`, `dummy_bits`
 * dummy cycles (rounded up to whole bytes, at most 32 bits), then `length` bytes into `rx`.
 * Interrupts are masked for the whole command. */
void hal_nor_bus_transfer_in(uint8_t opcode, bool has_address, uint32_t address, uint8_t dummy_bits, uint8_t *rx,
                             uint32_t length);

/* One command that writes: `opcode`, the 24-bit `address`, then `length` bytes from `tx`.
 * Interrupts are masked for the whole command. */
void hal_nor_bus_transfer_out(uint8_t opcode, uint32_t address, const uint8_t *tx, uint32_t length);

#endif /* HAL_NOR_BUS_H */
