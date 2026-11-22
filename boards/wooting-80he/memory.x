MEMORY
{
  FLASH (rx) : ORIGIN = 0x08008000, LENGTH = 0x000F6000
  RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0x00030000
}

SECTIONS
{
  .app_descriptor ALIGN(ADDR(.vector_table) + SIZEOF(.vector_table), 64) :
  {
    KEEP(*(.app_descriptor));
  } > FLASH
}
INSERT AFTER .vector_table;

_stext = ADDR(.app_descriptor) + SIZEOF(.app_descriptor);
