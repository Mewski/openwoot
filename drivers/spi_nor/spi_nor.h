/*
 * spi_nor.h - the command set of a serial NOR flash chip of the W25Q / GD25Q kind, as far as
 * the driver in spi_nor.c uses it.
 *
 * Depends on: nothing.
 */
#ifndef SPI_NOR_H
#define SPI_NOR_H

/* Opcodes. */
#define SPI_NOR_CMD_PAGE_PROGRAM    0x02u   /* + 24-bit address + up to one page of data */
#define SPI_NOR_CMD_READ_STATUS     0x05u   /* status register 1 */
#define SPI_NOR_CMD_WRITE_ENABLE    0x06u
#define SPI_NOR_CMD_FAST_READ       0x0Bu   /* + 24-bit address + 8 dummy bits */
#define SPI_NOR_CMD_SECTOR_ERASE    0x20u   /* 4 KiB, + 24-bit address */
#define SPI_NOR_CMD_UNIQUE_ID       0x4Bu   /* + 32 dummy bits, 8 bytes */
#define SPI_NOR_CMD_READ_ID         0x90u   /* + 24-bit address 0, manufacturer and device id */

#define SPI_NOR_FAST_READ_DUMMY_BITS    8u
#define SPI_NOR_UNIQUE_ID_DUMMY_BITS    32u

#define SPI_NOR_STATUS_BUSY         0x01u   /* status register bit 0: write in progress */
#define SPI_NOR_PAGE_SIZE           256u    /* a page program does not cross this boundary */

#endif /* SPI_NOR_H */
