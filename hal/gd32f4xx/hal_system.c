/*
 * hal_system.c - the GD32F4xx microcontroller as a whole (interface: hal/hal_system.h):
 * shared clocks, reset, and the messages exchanged with the bootloader.
 *
 * The bootloader and the firmware talk through the RTC backup registers, which survive a
 * reset and are write protected except between pmu_backup_write_enable and
 * pmu_backup_write_disable:
 *   RTC_BKP0  HAL_SYSTEM_STAY_IN_UPDATE_MODE: the bootloader stays in update mode
 *   RTC_BKP1  HAL_SYSTEM_FORMAT_REQUEST: format the settings volume (written by the
 *             bootloader, cleared by the firmware)
 *   RTC_BKP2  the option byte of the host command that caused the reset
 * The values match stock firmware 2.14.1.
 *
 * Depends on: the GD32F4xx library (rcu, pmu) and CMSIS (NVIC_SystemReset).
 */
#include "gd32f4xx.h"

#include "hal/hal_system.h"

#define HAL_SYSTEM_STAY_IN_UPDATE_MODE 0x504u
#define HAL_SYSTEM_FORMAT_REQUEST      0x420u

/* Clocks of GPIOA, GPIOB, GPIOC, SYSCFG and DMA0. */
void hal_system_init(void)
{
    rcu_periph_clock_enable(RCU_GPIOA);
    rcu_periph_clock_enable(RCU_GPIOB);
    rcu_periph_clock_enable(RCU_GPIOC);
    rcu_periph_clock_enable(RCU_SYSCFG);
    rcu_periph_clock_enable(RCU_DMA0);
}

/* NVIC_SystemReset ends in an endless loop that the compiler does not know to be one. */
void hal_system_reset(void)
{
    NVIC_SystemReset();
    __builtin_unreachable();
}

/* RTC_BKP2 is written only if `option` is not 0. */
void hal_system_restart(uint8_t option)
{
    if (option != 0) {
        pmu_backup_write_enable();
        RTC_BKP2 = option;
        pmu_backup_write_disable();
    }
    hal_system_reset();
}

void hal_system_enter_update_mode(uint8_t option)
{
    pmu_backup_write_enable();
    RTC_BKP0 = HAL_SYSTEM_STAY_IN_UPDATE_MODE;
    RTC_BKP2 = option;
    pmu_backup_write_disable();
    hal_system_reset();
}

bool hal_system_take_format_request(void)
{
    if (RTC_BKP1 != HAL_SYSTEM_FORMAT_REQUEST) {
        return false;
    }
    pmu_backup_write_enable();
    RTC_BKP1 = 0;
    pmu_backup_write_disable();
    return true;
}
