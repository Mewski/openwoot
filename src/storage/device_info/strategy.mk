# Strategy slot device_info: where the factory device information record is kept and whether
# this firmware can rewrite it. A board selects one variant with STRATEGY_device_info;
# BOARD_DEVICE_INFO_ADDR is the address of the record in the medium the variant names.
#
# Interface: device_info.h in this directory. Every sub-directory is a variant with the file
# device_info.c (storage_read_device_info, storage_write_device_info); the record format they
# share is src/storage/storage_device_info.c.
