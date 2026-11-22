/*
 * hal_mcu.c - facts and services of the GD32F30x chip (interface: hal/hal_mcu.h): unique id,
 * read protection, reading and programming of its own flash, user option byte.
 *
 * The firmware does not write the flash of this family: the flash controller driver of the
 * library is not part of the build (hal.mk). Matches stock firmware 2.14.1.
 *
 * Depends on: the GD32F30x library headers (the register of the flash controller that is
 * read).
 */
#include <string.h>

#include "gd32f30x.h"

#include "hal/hal_mcu.h"

#define HAL_MCU_UNIQUE_ID_ADDR       0x1FFFF7E8u /* 96-bit unique id */
#define HAL_MCU_PROTECTION_ADDR      0x1FFFF800u /* security protection option byte, 16-bit word */
#define HAL_MCU_PROTECTION_NONE      0xA5u       /* its low byte while the flash can be read out */
#define HAL_MCU_USER_OPTION_SHIFT    10u         /* first user data option byte in FMC_OBSTAT */

void hal_mcu_read_unique_id(uint32_t id[3])
{
    id[0] = REG32(HAL_MCU_UNIQUE_ID_ADDR);
    id[1] = REG32(HAL_MCU_UNIQUE_ID_ADDR + 4u);
    id[2] = REG32(HAL_MCU_UNIQUE_ID_ADDR + 8u);
}

bool hal_mcu_is_read_protected(uint8_t *code)
{
    uint8_t protection = (uint8_t)REG16(HAL_MCU_PROTECTION_ADDR);

    if (protection == HAL_MCU_PROTECTION_NONE) {
        return false;
    }
    *code = protection;
    return true;
}

/* The program flash is mapped into the address space. */
void hal_mcu_flash_read(uint32_t address, uint8_t *data, uint32_t length)
{
    memcpy(data, (const void *)(uintptr_t)address, length);
}

/* Leaves the flash untouched. */
bool hal_mcu_flash_write_page(uint32_t address, const uint8_t *data, uint32_t length)
{
    return false;
}

uint8_t hal_mcu_read_user_option(void)
{
    return (uint8_t)(FMC_OBSTAT >> HAL_MCU_USER_OPTION_SHIFT);
}
