/*
 * Linker script for the Wooting 80HE application image: the memory regions of the board. The
 * section layout is that of the microcontroller family (sections.ld in its hal/ directory).
 *
 * Memory layout (the same as stock firmware 2.14.1, so the factory bootloader starts this image):
 *   0x08000000-0x08007FFF  factory bootloader (never linked or flashed from here)
 *   0x08008000             application vector table, .text, .rodata, .data load image
 *   0x080FE000-0x080FFFFF  reserved: the firmware mirrors the serial number here
 *   0x20000000             .data, .bss, heap (grows up from `end`), stack (down from 0x20030000)
 *
 * The numbers repeat BOARD_APP_BASE, BOARD_FLASH_END and BOARD_RAM_END of board.h;
 * tools/check_tree.py compares them.
 */

MEMORY
{
  FLASH (rx)  : ORIGIN = 0x08008000, LENGTH = 0x000F6000
  RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 0x00030000
}

INCLUDE sections.ld
