MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* These values correspond to the NRF52832 with SoftDevices S132 7.3.0 */
  FLASH :  ORIGIN = 0x00027000, LENGTH = 868K
  RAM :    ORIGIN = 0x2000f588, LENGTH = 128K
  PANDUMP: ORIGIN = 0x2002fC00, LENGTH = 1K
}

_panic_dump_start = ORIGIN(PANDUMP);
_panic_dump_end   = ORIGIN(PANDUMP) + LENGTH(PANDUMP);