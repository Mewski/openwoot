use super::sealed;
use crate::pac;

macro_rules! interrupts {
    ($($irq:ident),* $(,)?) => {
        $(
            #[allow(non_camel_case_types, reason = "the type name is the interrupt name")]
            #[derive(Debug)]
            pub enum $irq {}

            impl sealed::Interrupt for $irq {}

            impl super::Interrupt for $irq {
                const IRQ: pac::Interrupt = pac::Interrupt::$irq;
            }
        )*
    };
}

interrupts!(
    WWDGT,
    RTC_T_tamper,
    RTC_WKUP,
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
    ADC,
    CAN0_TX,
    CAN0_RX0,
    CAN0_RX1,
    CAN0_EWMC,
    EXTI_line9_5,
    TIMER0_BRK_TIMER8,
    TIMER0_UP_TIMER9,
    TIMER0_TRG_CMT_TIMER10,
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
    OTG_FS_WKUP,
    TIMER7_BRK_TIMER11,
    TIMER7_UP_TIMER12,
    TIMER7_TRG_CMT_TIMER13,
    TIMER7_CC,
    DMA0_Channel7,
    EXMC,
    SDIO,
    SPI2,
    UART3,
    TIMER5_DAC,
    TIMER6,
    DMA1_Channel0,
    DMA1_Channel1,
    DMA1_Channel2,
    DMA1_Channel3,
    DMA1_Channel4,
    ENET,
    ENET_WKUP,
    CAN1_TX,
    CAN1_RX0,
    CAN1_RX1,
    CAN1_EWMC,
    OTG_FS,
    DMA1_Channel5,
    DMA1_Channel6,
    DMA1_Channel7,
    USART5,
    I2C2_EV,
    I2C2_ER,
    USBHS_EP1_Out,
    USBHS_EP1_In,
    USBHS_WKUP,
    USBHS,
    DCI,
    TRNG,
    UART6,
    UART7,
    SPI3,
    SPI4,
    SPI5,
    TLI,
    TLI_ER,
    IPA,
);
