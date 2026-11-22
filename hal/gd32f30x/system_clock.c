/*
 * system_clock.c - the clock tree of the GD32F30x family, set up by Reset_Handler before the
 * C library is initialised: SystemInit and the library variable SystemCoreClock.
 *
 * This is a copy of system_gd32f30x.c (GD32F30x firmware library V2.1.3,
 * Copyright (c) 2012 ARM LIMITED / GigaDevice, BSD 3-Clause) reduced to one clock
 * configuration (__SYSTEM_CLOCK_120M_PLL_IRC8M) of the high-density and extra-density devices
 * and extended by the store at the end of SystemInit. It replaces
 * third_party/gd32f30x/device/system_gd32f30x.c, which must not be compiled.
 *
 * Result: the internal 8 MHz oscillator IRC8M / 2 * 30 -> CK_SYS = CK_AHB = CK_APB2 = 120 MHz,
 * CK_APB1 = 60 MHz, PMU high-drive mode on. The crystal oscillator is never switched on. No
 * flash wait states are configured (the library function has none either).
 *
 * Depends on: the GD32F30x library headers and BOARD_APP_BASE of board.h.
 */
#include "gd32f30x.h"

#include "board.h"

#define SYSTEM_CLOCK_HZ 120000000u /* CK_SYS behind the PLL */

/* The V2.1.3 RCU_MODIFY macro: halve the AHB clock and wait 20000 iterations before the
 * clock source is switched. */
#define RCU_MODIFY      {volatile uint32_t i; \
                         RCU_CFG0 |= RCU_AHB_CKSYS_DIV2; \
                         for(i=0;i<20000;i++);}

/* Core clock in Hz (system_gd32f30x.h). */
uint32_t SystemCoreClock = SYSTEM_CLOCK_HZ;

/* Switches the system clock to the PLL: IRC8M / 2 * 30 = 120 MHz, AHB / APB prescalers,
 * PMU high-drive mode. Hangs if the internal oscillator does not become stable. */
static void system_clock_120m_irc8m(void)
{
    uint32_t timeout = 0U;
    uint32_t stab_flag = 0U;

    /* enable IRC8M */
    RCU_CTL |= RCU_CTL_IRC8MEN;

    /* wait until IRC8M is stable or the startup time is longer than IRC8M_STARTUP_TIMEOUT (0x500) */
    do {
        timeout++;
        stab_flag = (RCU_CTL & RCU_CTL_IRC8MSTB);
    } while ((0U == stab_flag) && (IRC8M_STARTUP_TIMEOUT != timeout));

    /* if fail: hang */
    if (0U == (RCU_CTL & RCU_CTL_IRC8MSTB)) {
        while (1) {
        }
    }

    /* LDO output voltage high mode (0xC000) */
    RCU_APB1EN |= RCU_APB1EN_PMUEN;
    PMU_CTL |= PMU_CTL_LDOVS;

    /* AHB = SYSCLK (ORs 0) */
    RCU_CFG0 |= RCU_AHB_CKSYS_DIV1;
    /* APB2 = AHB (ORs 0) */
    RCU_CFG0 |= RCU_APB2_CKAHB_DIV1;
    /* APB1 = AHB / 2 (0x400) */
    RCU_CFG0 |= RCU_APB1_CKAHB_DIV2;

    /* CK_PLL = (CK_IRC8M / 2) * 30 = 120 MHz: the PLL source bit stays 0, multiplier field
     * 0x08340000 */
    RCU_CFG0 &= ~(RCU_CFG0_PLLMF | RCU_CFG0_PLLMF_4 | RCU_CFG0_PLLMF_5);
    RCU_CFG0 |= RCU_PLL_MUL30;

    /* enable PLL and wait until it is stable */
    RCU_CTL |= RCU_CTL_PLLEN;
    while (0U == (RCU_CTL & RCU_CTL_PLLSTB)) {
    }

    /* enable the high-drive to extend the clock frequency to 120 MHz */
    PMU_CTL |= PMU_CTL_HDEN;
    while (0U == (PMU_CS & PMU_CS_HDRF)) {
    }

    /* select the high-drive mode */
    PMU_CTL |= PMU_CTL_HDS;
    while (0U == (PMU_CS & PMU_CS_HDSRF)) {
    }

    /* select PLL as system clock (two separate writes) */
    RCU_CFG0 &= ~RCU_CFG0_SCS;
    RCU_CFG0 |= RCU_CKSYSSRC_PLL;

    /* wait until PLL is selected as system clock */
    while (0U == (RCU_CFG0 & RCU_SCSS_PLL)) {
    }
}

/* Resets the clock configuration, runs the clock setup and sets the vector table address.
 * Called only from the assembly body of Reset_Handler, which LTO cannot see: keep it. */
__attribute__((used)) void SystemInit(void)
{
    /* FPU settings: CP10 and CP11 full access (SCB->CPACR |= 0x00F00000) */
#if (__FPU_PRESENT == 1) && (__FPU_USED == 1)
    SCB->CPACR |= ((3UL << 10 * 2) | (3UL << 11 * 2));
#endif

    /* reset the RCU clock configuration to the default reset state */
    RCU_CTL |= RCU_CTL_IRC8MEN;

    RCU_MODIFY

    RCU_CFG0 &= ~RCU_CFG0_SCS;

    /* reset HXTALEN, CKMEN and PLLEN bits */
    RCU_CTL &= ~(RCU_CTL_PLLEN | RCU_CTL_CKMEN | RCU_CTL_HXTALEN);

    /* disable all interrupts and clear their flags */
    RCU_INT = 0x009F0000U;

    /* reset HXTALBPS bit */
    RCU_CTL &= ~(RCU_CTL_HXTALBPS);

    /* reset CFG0 and CFG1 registers */
    RCU_CFG0 = 0x00000000U;
    RCU_CFG1 = 0x00000000U;

    /* system_clock_config() of the library selects exactly one function */
    system_clock_120m_irc8m();

    /* Addition to the library function: the application's vector table lives behind the
     * bootloader. Matches stock firmware 2.14.1. */
    SCB->VTOR = BOARD_APP_BASE;
}
