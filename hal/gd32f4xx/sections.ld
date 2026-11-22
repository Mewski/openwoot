/*
 * sections.ld - section layout of an application image of the GD32F4xx family.
 *
 * Included by the linker script of a board (boards/<board>/linker.ld), which defines the
 * memory regions FLASH (the application area behind the bootloader) and RAM before the
 * INCLUDE line. The image starts with the vector table (hal/gd32f4xx/vectors.c); the code
 * follows on the next 64-byte boundary.
 *
 * Symbols for hal/gd32f4xx/startup.c and vectors.c: _estack, _sidata, _sdata, _edata, _sbss,
 * _ebss.
 */

ENTRY(Reset_Handler)

_estack = ORIGIN(RAM) + LENGTH(RAM);

SECTIONS
{
  .isr_vector :
  {
    . = ALIGN(4);
    KEEP(*(.isr_vector))
    . = ALIGN(4);
  } > FLASH

  .text :
  {
    . = ALIGN(64);
    *(.text)
    *(.text*)
    *(.glue_7)
    *(.glue_7t)
    *(.eh_frame)
    KEEP(*(.init))
    KEEP(*(.fini))
    . = ALIGN(4);
    _etext = .;
  } > FLASH

  .rodata :
  {
    . = ALIGN(16);
    *(.rodata)
    *(.rodata*)
    . = ALIGN(4);
    /* Registration lists (src/core/registry.h): the entries of each list, sorted by section
     * name, lie between the two markers of the list. */
    KEEP(*(SORT(.registry.*)))
    . = ALIGN(4);
  } > FLASH

  .ARM.extab : { *(.ARM.extab* .gnu.linkonce.armextab.*) } > FLASH
  .ARM.exidx :
  {
    __exidx_start = .;
    *(.ARM.exidx* .gnu.linkonce.armexidx.*)
    __exidx_end = .;
  } > FLASH

  .preinit_array :
  {
    PROVIDE_HIDDEN(__preinit_array_start = .);
    KEEP(*(.preinit_array*))
    PROVIDE_HIDDEN(__preinit_array_end = .);
  } > FLASH

  .init_array :
  {
    PROVIDE_HIDDEN(__init_array_start = .);
    KEEP(*(SORT(.init_array.*)))
    KEEP(*(.init_array*))
    PROVIDE_HIDDEN(__init_array_end = .);
  } > FLASH

  .fini_array :
  {
    PROVIDE_HIDDEN(__fini_array_start = .);
    KEEP(*(SORT(.fini_array.*)))
    KEEP(*(.fini_array*))
    PROVIDE_HIDDEN(__fini_array_end = .);
  } > FLASH

  /* Load address of .data; Reset_Handler copies [_sidata, ...) to [_sdata, _edata). */
  _sidata = LOADADDR(.data);

  .data :
  {
    . = ALIGN(4);
    _sdata = .;
    *(.data)
    *(.data*)
    . = ALIGN(4);
    _edata = .;
  } > RAM AT > FLASH

  .bss (NOLOAD) :
  {
    . = ALIGN(4);
    _sbss = .;
    __bss_start__ = _sbss;
    *(.bss)
    *(.bss*)
    *(COMMON)
    . = ALIGN(4);
    _ebss = .;
    __bss_end__ = _ebss;
  } > RAM

  /* newlib's _sbrk (libnosys) starts the heap at `end`; there is no heap limit. */
  PROVIDE(end = _ebss);
  PROVIDE(_end = _ebss);

  /DISCARD/ :
  {
    libc.a:*(.ARM.attributes)
  }

  .ARM.attributes 0 : { *(.ARM.attributes) }
}
