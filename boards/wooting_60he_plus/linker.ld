/*
 * Linker script for the Wooting 60HE+ application image: the memory regions of the board. The
 * section layout is that of the microcontroller family (sections.ld in its hal/ directory).
 *
 * Memory layout (the same as stock firmware 2.14.1):
 *   0x08000000-0x08006FFF  factory bootloader (never linked or flashed from here)
 *   0x08007000             application vector table, .text, .rodata, .data load image
 *   0x080FD000-0x080FFFFF  not part of the image: serial number page (0x080FD000), one page
 *                          that the firmware does not use (0x080FE000) and the factory device
 *                          information (0x080FF000). openwoot reads these pages and never
 *                          writes them.
 *   0x20000000             .data, .bss, heap (grows up from `end`), stack (down from 0x20018000)
 *
 * The numbers repeat BOARD_APP_BASE, BOARD_FLASH_END and BOARD_RAM_END of board.h;
 * tools/check_tree.py compares them.
 */

MEMORY
{
  FLASH (rx)  : ORIGIN = 0x08007000, LENGTH = 0x000F6000
  RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 0x00018000
}

INCLUDE sections.ld
