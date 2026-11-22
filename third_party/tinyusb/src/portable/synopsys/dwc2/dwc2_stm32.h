/*
 * The MIT License (MIT)
 *
 * Copyright (c) 2021, Ha Thach (tinyusb.org)
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 *
 * This file is part of the TinyUSB stack.
 */

#ifndef _DWC2_STM32_H_
#define _DWC2_STM32_H_

#ifdef __cplusplus
 extern "C" {
#endif

// EP_MAX       : Max number of bi-directional endpoints including EP0
// EP_FIFO_SIZE : Size of dedicated USB SRAM
#if CFG_TUSB_MCU == OPT_MCU_STM32F1
  #include "stm32f1xx.h"
  #define EP_MAX_FS       4
  #define EP_FIFO_SIZE_FS 1280

#elif CFG_TUSB_MCU == OPT_MCU_STM32F2
  #include "stm32f2xx.h"
  #define EP_MAX_FS       USB_OTG_FS_MAX_IN_ENDPOINTS
  #define EP_FIFO_SIZE_FS USB_OTG_FS_TOTAL_FIFO_SIZE

#elif CFG_TUSB_MCU == OPT_MCU_STM32F4
  /* openwoot: the MCU is a GigaDevice GD32F425, whose USBFS/USBHS cores sit at
   * the STM32F4 OTG_FS/OTG_HS addresses. The STM32F4 binding is used with the
   * GD32 device header instead of stm32f4xx.h. The keyboard uses the
   * high-speed core: register base 0x40040000, 6 endpoints, 4096 bytes of FIFO
   * RAM, IRQ 77 (port 1). The full-speed values (port 0, IRQ 67) are the
   * STM32F4 ones and are not used by this firmware. */
  #include "gd32f4xx.h"
  #define USB_OTG_FS_PERIPH_BASE        USBFS_BASE    // 0x50000000
  #define USB_OTG_HS_PERIPH_BASE        USBHS_BASE    // 0x40040000
  #define USB_OTG_FS_MAX_IN_ENDPOINTS   4U
  #define USB_OTG_FS_TOTAL_FIFO_SIZE    1280U
  #define USB_OTG_HS_MAX_IN_ENDPOINTS   6U
  #define USB_OTG_HS_TOTAL_FIFO_SIZE    4096U
  #define OTG_FS_IRQn                   USBFS_IRQn    // 67
  #define OTG_HS_IRQn                   USBHS_IRQn    // 77
  #define EP_MAX_FS       USB_OTG_FS_MAX_IN_ENDPOINTS
  #define EP_FIFO_SIZE_FS USB_OTG_FS_TOTAL_FIFO_SIZE
  #define EP_MAX_HS       USB_OTG_HS_MAX_IN_ENDPOINTS
  #define EP_FIFO_SIZE_HS USB_OTG_HS_TOTAL_FIFO_SIZE

#elif CFG_TUSB_MCU == OPT_MCU_STM32H7
  #include "stm32h7xx.h"
  #define EP_MAX_FS       9
  #define EP_FIFO_SIZE_FS 4096
  #define EP_MAX_HS       9
  #define EP_FIFO_SIZE_HS 4096

#elif CFG_TUSB_MCU == OPT_MCU_STM32F7
  #include "stm32f7xx.h"
  #define EP_MAX_FS       6
  #define EP_FIFO_SIZE_FS 1280
  #define EP_MAX_HS       9
  #define EP_FIFO_SIZE_HS 4096

#elif CFG_TUSB_MCU == OPT_MCU_STM32L4
  #include "stm32l4xx.h"
  #define EP_MAX_FS       6
  #define EP_FIFO_SIZE_FS 1280

#else
  #error "Unsupported MCUs"
#endif

// On STM32 we associate Port0 to OTG_FS, and Port1 to OTG_HS
#if TUD_OPT_RHPORT == 0
  #define DWC2_REG_BASE       USB_OTG_FS_PERIPH_BASE
  #define DWC2_EP_MAX         EP_MAX_FS
  #define DWC2_EP_FIFO_SIZE   EP_FIFO_SIZE_FS
  #define RHPORT_IRQn         OTG_FS_IRQn

#else
  #define DWC2_REG_BASE       USB_OTG_HS_PERIPH_BASE
  #define DWC2_EP_MAX         EP_MAX_HS
  #define DWC2_EP_FIFO_SIZE   EP_FIFO_SIZE_HS
  #define RHPORT_IRQn         OTG_HS_IRQn

#endif

extern uint32_t SystemCoreClock;

/* openwoot: the interrupt is selected by the port number at run time, port 0 =
 * full-speed core, port 1 = high-speed core, anything else (0xFF before
 * tud_init) does nothing. */
TU_ATTR_ALWAYS_INLINE
static inline void dwc2_dcd_int_enable(uint8_t rhport)
{
  if ( rhport == 0 )
  {
    NVIC_EnableIRQ(OTG_FS_IRQn);
  }
  else if ( rhport == 1 )
  {
    NVIC_EnableIRQ(OTG_HS_IRQn);
  }
}

TU_ATTR_ALWAYS_INLINE
static inline void dwc2_dcd_int_disable (uint8_t rhport)
{
  if ( rhport == 0 )
  {
    NVIC_DisableIRQ(OTG_FS_IRQn);
  }
  else if ( rhport == 1 )
  {
    NVIC_DisableIRQ(OTG_HS_IRQn);
  }
}

TU_ATTR_ALWAYS_INLINE
static inline void dwc2_remote_wakeup_delay(void)
{
  // try to delay for 1 ms
  uint32_t count = SystemCoreClock / 1000;
  while ( count-- ) __NOP();
}

// MCU specific PHY init, called BEFORE core reset
static inline void dwc2_phy_init(dwc2_regs_t * dwc2, uint8_t hs_phy_type)
{
  if ( hs_phy_type == HS_PHY_TYPE_NONE )
  {
    // Enable on-chip FS PHY
    dwc2->stm32_gccfg |= STM32_GCCFG_PWRDWN;
  }else
  {
    // Disable FS PHY
    dwc2->stm32_gccfg &= ~STM32_GCCFG_PWRDWN;

    /* openwoot: make sure the external ULPI PHY is selected (GD32 USBHS_GUSBCS
     * bit 6 EMBPHY = 0), as a separate register write after the GCCFG update. */
    dwc2->gusbcfg &= ~GUSBCFG_PHYSEL;

    // Enable on-chip HS PHY
    if (hs_phy_type == HS_PHY_TYPE_UTMI || hs_phy_type == HS_PHY_TYPE_UTMI_ULPI)
    {
#ifdef USB_HS_PHYC
      // Enable UTMI HS PHY
      dwc2->stm32_gccfg |= STM32_GCCFG_PHYHSEN;

      // Enable LDO
      USB_HS_PHYC->USB_HS_PHYC_LDO |= USB_HS_PHYC_LDO_ENABLE;

      // Wait until LDO ready
      while ( 0 == (USB_HS_PHYC->USB_HS_PHYC_LDO & USB_HS_PHYC_LDO_STATUS) ) {}

      uint32_t phyc_pll = 0;

      // TODO Try to get HSE_VALUE from registers instead of depending CFLAGS
      switch ( HSE_VALUE )
      {
        case 12000000: phyc_pll = USB_HS_PHYC_PLL1_PLLSEL_12MHZ   ; break;
        case 12500000: phyc_pll = USB_HS_PHYC_PLL1_PLLSEL_12_5MHZ ; break;
        case 16000000: phyc_pll = USB_HS_PHYC_PLL1_PLLSEL_16MHZ   ; break;
        case 24000000: phyc_pll = USB_HS_PHYC_PLL1_PLLSEL_24MHZ   ; break;
        case 25000000: phyc_pll = USB_HS_PHYC_PLL1_PLLSEL_25MHZ   ; break;
        case 32000000: phyc_pll = USB_HS_PHYC_PLL1_PLLSEL_Msk     ; break; // Value not defined in header
        default:
          TU_ASSERT(false, );
      }
      USB_HS_PHYC->USB_HS_PHYC_PLL = phyc_pll;

      // Control the tuning interface of the High Speed PHY
      // Use magic value (USB_HS_PHYC_TUNE_VALUE) from ST driver for F7
      USB_HS_PHYC->USB_HS_PHYC_TUNE |= 0x00000F13U;

      // Enable PLL internal PHY
      USB_HS_PHYC->USB_HS_PHYC_PLL |= USB_HS_PHYC_PLL_PLLEN;
#endif
    }
  }
}

// MCU specific PHY update, it is called AFTER init() and core reset
static inline void dwc2_phy_update(dwc2_regs_t * dwc2, uint8_t hs_phy_type)
{
  // used to set turnaround time for fullspeed, nothing to do in highspeed mode
  if ( hs_phy_type == HS_PHY_TYPE_NONE )
  {
    // Turnaround timeout depends on the AHB clock dictated by STM32 Reference Manual
    uint32_t turnaround;

    if ( SystemCoreClock >= 32000000u )
      turnaround = 0x6u;
    else if ( SystemCoreClock >= 27500000u )
      turnaround = 0x7u;
    else if ( SystemCoreClock >= 24000000u )
      turnaround = 0x8u;
    else if ( SystemCoreClock >= 21800000u )
      turnaround = 0x9u;
    else if ( SystemCoreClock >= 20000000u )
      turnaround = 0xAu;
    else if ( SystemCoreClock >= 18500000u )
      turnaround = 0xBu;
    else if ( SystemCoreClock >= 17200000u )
      turnaround = 0xCu;
    else if ( SystemCoreClock >= 16000000u )
      turnaround = 0xDu;
    else if ( SystemCoreClock >= 15000000u )
      turnaround = 0xEu;
    else
      turnaround = 0xFu;

    dwc2->gusbcfg = (dwc2->gusbcfg & ~GUSBCFG_TRDT_Msk) | (turnaround << GUSBCFG_TRDT_Pos);
  }
}

#ifdef __cplusplus
}
#endif

#endif /* _DWC2_STM32_H_ */
