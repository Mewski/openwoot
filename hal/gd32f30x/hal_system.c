/*
 * hal_system.c - the GD32F30x microcontroller as a whole (interface: hal/hal_system.h):
 * shared clocks, reset, and the messages exchanged with the bootloader.
 *
 * The bootloader and the firmware talk through the backup data registers, which survive a
 * reset, are reached only while the backup interface clock is on, and are write protected
 * except between pmu_backup_write_enable and pmu_backup_write_disable:
 *   BKP_DATA0  HAL_SYSTEM_STAY_IN_UPDATE_MODE: the bootloader stays in update mode
 *   BKP_DATA1  HAL_SYSTEM_FORMAT_REQUEST: format the settings volume (written by the
 *              bootloader, cleared by the firmware)
 *   BKP_DATA2  the option byte of the host command that caused the reset
 * Every access first sets the tamper event reset bit (BKP_TPCS_TER). The values and the
 * sequences match stock firmware 2.14.1.
 *
 * Depends on: the GD32F30x library (rcu, pmu, bkp registers) and CMSIS (NVIC_SystemReset).
 */
#include "gd32f30x.h"

#include "hal/hal_system.h"

#define HAL_SYSTEM_STAY_IN_UPDATE_MODE 0x504u
#define HAL_SYSTEM_FORMAT_REQUEST      0x420u

/* Clocks of GPIOA, GPIOB, GPIOC, AFIO and DMA0. */
void hal_system_init(void)
{
    rcu_periph_clock_enable(RCU_GPIOA);
    rcu_periph_clock_enable(RCU_GPIOB);
    rcu_periph_clock_enable(RCU_GPIOC);
    rcu_periph_clock_enable(RCU_AF);
    rcu_periph_clock_enable(RCU_DMA0);
}

/* NVIC_SystemReset ends in an endless loop that the compiler does not know to be one. */
void hal_system_reset(void)
{
    NVIC_SystemReset();
    __builtin_unreachable();
}

/* BKP_DATA2 is written only if `option` is not 0. The backup interface clock stays on. */
void hal_system_restart(uint8_t option)
{
    if (option != 0) {
        rcu_periph_clock_enable(RCU_BKPI);
        BKP_TPCS |= BKP_TPCS_TER;
        pmu_backup_write_enable();
        BKP_DATA2 = option;
        pmu_backup_write_disable();
    }
    hal_system_reset();
}

/* The backup interface clock stays on. */
void hal_system_enter_update_mode(uint8_t option)
{
    rcu_periph_clock_enable(RCU_BKPI);
    BKP_TPCS |= BKP_TPCS_TER;
    pmu_backup_write_enable();
    BKP_DATA0 = HAL_SYSTEM_STAY_IN_UPDATE_MODE;
    BKP_DATA2 = option;
    pmu_backup_write_disable();
    hal_system_reset();
}

/* The write protection is left off after the request is cleared; the backup interface
 * clock is switched off again in both cases. */
bool hal_system_take_format_request(void)
{
    rcu_periph_clock_enable(RCU_BKPI);
    BKP_TPCS |= BKP_TPCS_TER;
    if (BKP_DATA1 != HAL_SYSTEM_FORMAT_REQUEST) {
        rcu_periph_clock_disable(RCU_BKPI);
        return false;
    }
    pmu_backup_write_enable();
    BKP_DATA1 = 0;
    rcu_periph_clock_disable(RCU_BKPI);
    return true;
}
