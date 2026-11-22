/*
 * vectors.c - the vector table of the GD32F30x family: 76 entries (initial stack pointer,
 * 15 system vectors, 60 interrupts), in the layout of the start-up file for high-density and
 * extra-density devices.
 *
 * Five vectors lead into the firmware: the 1 ms tick, the conversion interrupt of the sensor
 * scan, the two interrupts of the USB controller and the overflow interrupt of the microsecond
 * timer. Every other vector that the family defines points at Default_Handler (startup.c); the
 * comment of such an entry names its interrupt.
 *
 * Depends on: the linker symbol _estack (sections.ld), Reset_Handler and Default_Handler
 * (startup.c), and the five handlers declared below. The table goes to the section .isr_vector,
 * which sections.ld places at the start of the application image.
 */
#include <stdint.h>

extern uint32_t _estack; /* end of RAM */

void Reset_Handler(void);
void Default_Handler(void);
void hal_time_tick_isr(void);
void hal_scan_conversion_isr(void);
void USBD_HP_CAN0_TX_IRQHandler(void);
void USBD_LP_CAN0_RX0_IRQHandler(void);
void TIMER4_IRQHandler(void);

typedef void (*vector_t)(void);

/* Reserved entries (7..10, 13) are zero. Entry 53 (interrupt 37, USART0) is zero as well.
 * Matches stock firmware 2.14.1. */
__attribute__((section(".isr_vector"), used))
const vector_t g_vector_table[76] = {
    [0]  = (vector_t)&_estack,
    [1]  = Reset_Handler,
    [2]  = Default_Handler,                     /* NMI */
    [3]  = Default_Handler,                     /* HardFault */
    [4]  = Default_Handler,                     /* MemManage */
    [5]  = Default_Handler,                     /* BusFault */
    [6]  = Default_Handler,                     /* UsageFault */
    [7]  = 0,
    [8]  = 0,
    [9]  = 0,
    [10] = 0,
    [11] = Default_Handler,                     /* SVC */
    [12] = Default_Handler,                     /* DebugMon */
    [13] = 0,
    [14] = Default_Handler,                     /* PendSV */
    [15] = hal_time_tick_isr,                   /* 1 ms tick */
    /* interrupts: index = 16 + interrupt number */
    [16] = Default_Handler,                     /* WWDGT */
    [17] = Default_Handler,                     /* LVD */
    [18] = Default_Handler,                     /* TAMPER */
    [19] = Default_Handler,                     /* RTC */
    [20] = Default_Handler,                     /* FMC */
    [21] = Default_Handler,                     /* RCU_CTC */
    [22] = Default_Handler,                     /* EXTI0 */
    [23] = Default_Handler,                     /* EXTI1 */
    [24] = Default_Handler,                     /* EXTI2 */
    [25] = Default_Handler,                     /* EXTI3 */
    [26] = Default_Handler,                     /* EXTI4 */
    [27] = Default_Handler,                     /* DMA0_Channel0 */
    [28] = Default_Handler,                     /* DMA0_Channel1 */
    [29] = Default_Handler,                     /* DMA0_Channel2 */
    [30] = Default_Handler,                     /* DMA0_Channel3 */
    [31] = Default_Handler,                     /* DMA0_Channel4 */
    [32] = Default_Handler,                     /* DMA0_Channel5 */
    [33] = Default_Handler,                     /* DMA0_Channel6 */
    [34] = hal_scan_conversion_isr,             /* interrupt 18: a conversion of the sensor scan is complete */
    [35] = USBD_HP_CAN0_TX_IRQHandler,          /* interrupt 19: USB full-speed controller */
    [36] = USBD_LP_CAN0_RX0_IRQHandler,         /* interrupt 20: USB full-speed controller */
    [37] = Default_Handler,                     /* CAN0_RX1 */
    [38] = Default_Handler,                     /* CAN0_EWMC */
    [39] = Default_Handler,                     /* EXTI5_9 */
    [40] = Default_Handler,                     /* TIMER0_BRK_TIMER8 */
    [41] = Default_Handler,                     /* TIMER0_UP_TIMER9 */
    [42] = Default_Handler,                     /* TIMER0_TRG_CMT_TIMER10 */
    [43] = Default_Handler,                     /* TIMER0_Channel */
    [44] = Default_Handler,                     /* TIMER1 */
    [45] = Default_Handler,                     /* TIMER2 */
    [46] = Default_Handler,                     /* TIMER3 */
    [47] = Default_Handler,                     /* I2C0_EV */
    [48] = Default_Handler,                     /* I2C0_ER */
    [49] = Default_Handler,                     /* I2C1_EV */
    [50] = Default_Handler,                     /* I2C1_ER */
    [51] = Default_Handler,                     /* SPI0 */
    [52] = Default_Handler,                     /* SPI1 */
    [53] = 0,
    [54] = Default_Handler,                     /* USART1 */
    [55] = Default_Handler,                     /* USART2 */
    [56] = Default_Handler,                     /* EXTI10_15 */
    [57] = Default_Handler,                     /* RTC_Alarm */
    [58] = Default_Handler,                     /* USBD_WKUP */
    [59] = Default_Handler,                     /* TIMER7_BRK_TIMER11 */
    [60] = Default_Handler,                     /* TIMER7_UP_TIMER12 */
    [61] = Default_Handler,                     /* TIMER7_TRG_CMT_TIMER13 */
    [62] = Default_Handler,                     /* TIMER7_Channel */
    [63] = Default_Handler,                     /* ADC2 */
    [64] = Default_Handler,                     /* EXMC */
    [65] = Default_Handler,                     /* SDIO */
    [66] = TIMER4_IRQHandler,                   /* interrupt 50: overflow of the microsecond timer */
    [67] = Default_Handler,                     /* SPI2 */
    [68] = Default_Handler,                     /* UART3 */
    [69] = Default_Handler,                     /* UART4 */
    [70] = Default_Handler,                     /* TIMER5 */
    [71] = Default_Handler,                     /* TIMER6 */
    [72] = Default_Handler,                     /* DMA1_Channel0 */
    [73] = Default_Handler,                     /* DMA1_Channel1 */
    [74] = Default_Handler,                     /* DMA1_Channel2 */
    [75] = Default_Handler,                     /* DMA1_Channel3_Channel4 */
};

_Static_assert(sizeof(g_vector_table) == 0x130, "vector table size: 76 vectors");
