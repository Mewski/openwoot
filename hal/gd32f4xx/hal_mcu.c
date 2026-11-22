/*
 * hal_mcu.c - facts and services of the GD32F4xx chip (interface: hal/hal_mcu.h): unique id,
 * read protection, reading and programming of its own flash, user option byte.
 *
 * Depends on: the GD32F4xx library (fmc).
 */
#include <string.h>

#include "gd32f4xx.h"

#include "hal/hal_mcu.h"

#define HAL_MCU_UNIQUE_ID_ADDR       0x1FFF7A10u /* 96-bit unique id */
#define HAL_MCU_PROTECTION_ADDR      0x1FFFC001u /* security protection option byte */
#define HAL_MCU_PROTECTION_NONE      0xAAu       /* its value while the flash can be read out */

void hal_mcu_read_unique_id(uint32_t id[3])
{
    id[0] = REG32(HAL_MCU_UNIQUE_ID_ADDR);
    id[1] = REG32(HAL_MCU_UNIQUE_ID_ADDR + 4u);
    id[2] = REG32(HAL_MCU_UNIQUE_ID_ADDR + 8u);
}

bool hal_mcu_is_read_protected(uint8_t *code)
{
    uint8_t protection = REG8(HAL_MCU_PROTECTION_ADDR);

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

/* Unlock, clear the END and WPERR flags, erase the page, program half-word by half-word
 * (at least one), lock. Nothing is verified. */
bool hal_mcu_flash_write_page(uint32_t address, const uint8_t *data, uint32_t length)
{
    uint32_t end = address + length;
    uint32_t cursor = address;

    fmc_unlock();
    fmc_flag_clear(FMC_FLAG_END);
    fmc_flag_clear(FMC_FLAG_WPERR);
    fmc_page_erase(address);

    do {
        uint16_t halfword;

        memcpy(&halfword, &data[cursor - address], sizeof(halfword));
        fmc_halfword_program(cursor, halfword);
        cursor += 2u;
    } while (cursor < end);

    fmc_lock();
    return true;
}

/* The family has no user option byte of this kind. */
uint8_t hal_mcu_read_user_option(void)
{
    return 0xFFu;
}
