use cortex_m::peripheral::NVIC;

use crate::pac;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(u8);

impl Priority {
    pub const HIGHEST: Self = Self(1);
    pub const KEYS: Self = Self(3);
    pub const LOWEST: Self = Self((1 << pac::NVIC_PRIO_BITS) - 1);
    pub(crate) const SCAN: Self = Self(0);
    pub const TIME: Self = Self::HIGHEST;
    pub const USB: Self = Self(2);

    #[must_use]
    pub const fn new(level: u8) -> Option<Self> {
        if level >= Self::HIGHEST.0 && level <= Self::LOWEST.0 {
            Some(Self(level))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn level(self) -> u8 {
        self.0
    }

    #[must_use]
    pub(crate) const fn to_nvic(self) -> u8 {
        self.0 << (8 - pac::NVIC_PRIO_BITS)
    }
}

pub trait Interrupt: sealed::Interrupt {
    const IRQ: pac::Interrupt;
}

pub trait Handler<Irq: Interrupt> {
    // SAFETY: implementations may assume that they run in the handler of `Irq`; callers must call
    // them only from there.
    #[expect(clippy::missing_safety_doc, reason = "the code has no doc comments")]
    unsafe fn on_interrupt();
}

// SAFETY: implemented only by `bind_interrupts!`, which defines the vector of `Irq` to call
// `Hook::on_interrupt`.
#[expect(clippy::missing_safety_doc, reason = "the code has no doc comments")]
pub unsafe trait Binding<Irq: Interrupt, Hook: Handler<Irq>> {}

#[macro_export]
macro_rules! bind_interrupts {
    ($visibility:vis struct $name:ident { $($irq:ident => $($handler:ty),+;)* }) => {
        #[derive(Clone, Copy, Debug)]
        $visibility struct $name;

        $(
            // SAFETY: the vector table names this symbol for the interrupt; one binding per
            // interrupt exists, or the link fails with a duplicate symbol.
            #[allow(non_snake_case, reason = "the vector name is the interrupt name")]
            #[unsafe(no_mangle)]
            unsafe extern "C" fn $irq() {
                $(
                    // SAFETY: this is the handler of the interrupt the handler is bound to.
                    unsafe {
                        <$handler as $crate::interrupt::Handler<
                            $crate::interrupt::typelevel::$irq,
                        >>::on_interrupt();
                    }
                )+
            }

            $(
                // SAFETY: the vector function above calls the handler.
                unsafe impl $crate::interrupt::Binding<$crate::interrupt::typelevel::$irq, $handler>
                    for $name
                {
                }
            )+
        )*
    };
}

pub(crate) fn set_priority(irq: pac::Interrupt, priority: Priority) {
    // SAFETY: only the NVIC priority byte of `irq` changes. Critical sections mask by BASEPRI, so a
    // changed priority cannot break one; `Priority` cannot express the scan priority outside this
    // crate.
    unsafe {
        cortex_m::Peripherals::steal()
            .NVIC
            .set_priority(irq, priority.to_nvic());
    }
}

pub(crate) fn enable(irq: pac::Interrupt, priority: Priority) {
    set_priority(irq, priority);
    // SAFETY: the handler of `irq` is defined by this crate or bound through `bind_interrupts!`,
    // and its priority was set first, so it never runs at the scan priority by accident.
    unsafe { NVIC::unmask(irq) };
}

mod sealed {
    pub trait Interrupt {}
}

pub mod typelevel {
    use super::{pac, sealed};

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
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    static CALLS: AtomicU32 = AtomicU32::new(0);

    struct Recorder;

    impl Handler<typelevel::DMA1_Channel3_4> for Recorder {
        // SAFETY: counting has no requirement on its caller.
        unsafe fn on_interrupt() {
            CALLS.fetch_add(1, Ordering::Relaxed);
        }
    }

    crate::bind_interrupts!(struct Irqs {
        DMA1_Channel3_4 => Recorder;
    });

    fn bound<Irq: Interrupt, Hook: Handler<Irq>>(
        _proof: impl Binding<Irq, Hook>,
    ) -> pac::Interrupt {
        Irq::IRQ
    }

    #[test]
    fn a_bound_vector_calls_its_handler() {
        assert_eq!(
            bound::<typelevel::DMA1_Channel3_4, Recorder>(Irqs),
            pac::Interrupt::DMA1_Channel3_4
        );
        // SAFETY: the test calls the vector function the way the core would.
        unsafe { DMA1_Channel3_4() };
        assert_eq!(CALLS.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn priorities_leave_level_zero_to_the_scan() {
        assert_eq!(Priority::new(0), None);
        assert_eq!(Priority::new(16), None);
        assert_eq!(Priority::new(1), Some(Priority::HIGHEST));
        assert_eq!(Priority::new(15), Some(Priority::LOWEST));
        assert_eq!(Priority::SCAN.to_nvic(), 0x00);
        assert_eq!(Priority::TIME.to_nvic(), 0x10);
        assert_eq!(Priority::LOWEST.to_nvic(), 0xF0);
        assert!(Priority::TIME < Priority::USB && Priority::USB < Priority::KEYS);
    }
}
