MEMORY
{
  FLASH (rx) : ORIGIN = 0x08007000, LENGTH = 0x000F6000
  RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0x00018000
  ZERO_WAIT : ORIGIN = 0x08000000, LENGTH = 0x00040000
}

SECTIONS
{
  .app_descriptor ALIGN(ADDR(.vector_table) + SIZEOF(.vector_table), 64) :
  {
    KEEP(*(.app_descriptor));
  } > FLASH
}
INSERT AFTER .vector_table;

_stext = ALIGN(ADDR(.app_descriptor) + SIZEOF(.app_descriptor), 8);
