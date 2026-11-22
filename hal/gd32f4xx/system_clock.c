/*
 * system_clock.c - the clock tree of the GD32F4xx family, set up by Reset_Handler before the
 * C library is initialised: SystemInit and the library variable SystemCoreClock.
 *
 * This is a copy of system_gd32f4xx.c (GD32F4xx firmware library V3.0.0,
 * Copyright (c) 2012 ARM LIMITED / GigaDevice, BSD 3-Clause) reduced to one clock
 * configuration (__SYSTEM_CLOCK_200M_PLL_25M_HXTAL) and extended by the two stores at the
 * end of SystemInit. It replaces third_party/gd32f4xx/device/system_gd32f4xx.c, which must
 * not be compiled.
 *
 * Result: HXTAL 25 MHz / 25 * 400 / 2 -> CK_SYS = CK_AHB = 200 MHz, CK_APB2 = 100 MHz,
 * CK_APB1 = 50 MHz, PMU high-drive mode on. No flash wait states are configured (the
 * library function has none either).
 *
 * Depends on: the GD32F4xx library headers and BOARD_APP_BASE of board.h.
 */
#include "gd32f4xx.h"

#include "board.h"

#define SYSTEM_CLOCK_HZ 200000000u /* CK_SYS behind the PLL */

/* The V3.0.0 RCU_MODIFY macro: step the AHB prescaler down (/2, then /4) with a
 * 50000-iteration delay after each step before the clock source is switched. */
#define RCU_MODIFY      {volatile uint32_t i; \
                         RCU_CFG0 |= RCU_AHB_CKSYS_DIV2; \
                         for(i=0;i<50000;i++); \
                         RCU_CFG0 |= RCU_AHB_CKSYS_DIV4; \
                         for(i=0;i<50000;i++);}

/* Core clock in Hz (system_gd32f4xx.h). Its only reader is TinyUSB's
 * dwc2_remote_wakeup_delay(). */
uint32_t SystemCoreClock = SYSTEM_CLOCK_HZ;

/* Switches the system clock to the PLL: HXTAL 25 MHz -> 200 MHz, AHB / APB prescalers,
 * PMU high-drive mode. Hangs if the crystal does not start. */
static void system_clock_200m_25m_hxtal(void)
{
    uint32_t timeout = 0U;
    uint32_t stab_flag = 0U;

    /* enable HXTAL */
    RCU_CTL |= RCU_CTL_HXTALEN;

    /* wait until HXTAL is stable or the startup time is longer than HXTAL_STARTUP_TIMEOUT (0xFFFF) */
    do {
        timeout++;
        stab_flag = (RCU_CTL & RCU_CTL_HXTALSTB);
    } while ((0U == stab_flag) && (HXTAL_STARTUP_TIMEOUT != timeout));

    /* if fail: hang */
    if (0U == (RCU_CTL & RCU_CTL_HXTALSTB)) {
        while (1) {
        }
    }

    RCU_APB1EN |= RCU_APB1EN_PMUEN;
    PMU_CTL |= PMU_CTL_LDOVS;               /* LDO output voltage select = 0b11 (0xC000) */

    /* AHB = SYSCLK (ORs 0) */
    RCU_CFG0 |= RCU_AHB_CKSYS_DIV1;
    /* APB2 = AHB / 2 (0x8000) */
    RCU_CFG0 |= RCU_APB2_CKAHB_DIV2;
    /* APB1 = AHB / 4 (0x1400) */
    RCU_CFG0 |= RCU_APB1_CKAHB_DIV4;

    /* main PLL: PSC = 25, N = 400, P = 2, source HXTAL, Q = 9 -> RCU_PLL = 0x09406419 */
    RCU_PLL = (25U | (400U << 6U) | (((2U >> 1U) - 1U) << 16U) |
               (RCU_PLLSRC_HXTAL) | (9U << 24U));

    /* enable PLL and wait until it is stable */
    RCU_CTL |= RCU_CTL_PLLEN;
    while (0U == (RCU_CTL & RCU_CTL_PLLSTB)) {
    }

    /* enable the high-drive to extend the clock frequency to 200 MHz */
    PMU_CTL |= PMU_CTL_HDEN;
    while (0U == (PMU_CS & PMU_CS_HDRF)) {
    }

    /* select the high-drive mode */
    PMU_CTL |= PMU_CTL_HDS;
    while (0U == (PMU_CS & PMU_CS_HDSRF)) {
    }

    /* select PLL as system clock (two separate writes) */
    RCU_CFG0 &= ~RCU_CFG0_SCS;
    RCU_CFG0 |= RCU_CKSYSSRC_PLLP;

    /* wait until PLL is selected as system clock */
    while (0U == (RCU_CFG0 & RCU_SCSS_PLLP)) {
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
    RCU_CTL |= RCU_CTL_IRC16MEN;

    RCU_MODIFY

    RCU_CFG0 &= ~RCU_CFG0_SCS;

    /* reset HXTALEN, CKMEN and PLLEN bits */
    RCU_CTL &= ~(RCU_CTL_PLLEN | RCU_CTL_CKMEN | RCU_CTL_HXTALEN);

    /* reset HXTALBPS bit */
    RCU_CTL &= ~(RCU_CTL_HXTALBPS);

    /* reset CFG0 register */
    RCU_CFG0 = 0x00000000U;

    /* wait until IRC16M is selected as system clock. RCU_SCSS_IRC16M is 0, so this is a
     * single read of RCU_CFG0 and the loop body never runs. */
    while (0 != (RCU_CFG0 & RCU_SCSS_IRC16M)) {
    }

    /* reset PLL register */
    RCU_PLL = 0x24003010U;

    /* disable all interrupts */
    RCU_INT = 0x00000000U;

    /* system_clock_config() of the library selects exactly one function */
    system_clock_200m_25m_hxtal();

    /* Additions to the library function: the application's vector table lives behind the
     * bootloader; then 0 is stored to SCB->ICSR (0xE000ED04), which has no effect because
     * every writable ICSR bit is write-one. Matches stock firmware 2.14.1. */
    SCB->VTOR = BOARD_APP_BASE;
    SCB->ICSR = 0U;
}
