/*
 * layout_variant.c, variant device_info of the strategy slot layout_variant: the physical
 * layout is field physical_layout (tag 3) of the device information record. Storing it
 * rewrites the record through storage_write_device_info, with the other fields as read.
 *
 * Depends on: storage/device_info/device_info.h.
 */
#include <string.h>

#include "storage/storage.h"

uint8_t storage_layout_variant_load(void)
{
    DeviceInfo info;

    memset(&info, 0, sizeof(info));
    storage_read_device_info(&info);
    return info.has_physical_layout ? (uint8_t)info.physical_layout : 0u;
}

void storage_layout_variant_store(uint8_t variant)
{
    DeviceInfo info;

    memset(&info, 0, sizeof(info));
    storage_read_device_info(&info);
    info.physical_layout = (PhysicalLayout)variant;
    info.has_physical_layout = true;
    storage_write_device_info(&info);
}
