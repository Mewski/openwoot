use super::{Interrupt, sealed};
use crate::pac;

macro_rules! interrupts {
    ($($irq:ident),* $(,)?) => {
        $(
            #[allow(non_camel_case_types, reason = "the type name is the interrupt name")]
            #[derive(Debug)]
            pub enum $irq {}

            impl sealed::Interrupt for $irq {}

            impl Interrupt for $irq {
                const IRQ: pac::Interrupt = pac::Interrupt::$irq;
            }
        )*
    };
}

interrupts!(
    WWDGT,
    Tamper,
    RTC,
    FMC,
    RCU_CTC,
    EXTI_Line0,
    EXTI_Line1,
    EXTI_Line2,
    EXTI_Line3,
    EXTI_Line4,
    DMA0_Channel0,
    DMA0_Channel1,
    DMA0_Channel2,
    DMA0_Channel3,
    DMA0_Channel4,
    DMA0_Channel5,
    DMA0_Channel6,
    ADC0_1,
    USBD_HP_CAN0_TX,
    USBD_LP_CAN0_RX0,
    CAN0_RX1,
    CAN0_EWMC,
    EXTI_line9_5,
    TIMER0_BRK,
    TIMER0_UP,
    TIMER0_TRG_CMT,
    TIMER0_CC,
    TIMER1,
    TIMER2,
    TIMER3,
    I2C0_EV,
    I2C0_ER,
    I2C1_EV,
    I2C1_ER,
    SPI0,
    SPI1,
    USART0,
    USART1,
    USART2,
    EXTI_line15_10,
    RTC_Alarm,
    USBD_WKUP,
    TIMER7_BRK,
    TIMER7_UP,
    TIMER7_TRG_CMT,
    TIMER7_CC,
    ADC2,
    EXMC,
    SDIO,
    SPI2,
    UART3,
    TIMER5,
    TIMER6,
    DMA1_Channel0,
    DMA1_Channel1,
    DMA1_Channel2,
    DMA1_Channel3_4,
);
