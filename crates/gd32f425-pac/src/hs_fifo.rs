#[repr(C)]
#[doc = "Register block"]
pub struct RegisterBlock {
    fifo: (),
}
impl RegisterBlock {
    #[doc = "0x00..0x18 - Data FIFO access window of endpoint or channel"]
    #[inline(always)]
    pub const fn fifo(&self, n: usize) -> &Fifo {
        #[allow(clippy::no_effect)]
        [(); 6][n];
        unsafe { &*core::ptr::from_ref(self).cast::<u8>().add(4096 * n).cast() }
    }

    #[doc = "Iterator for array of:"]
    #[doc = "0x00..0x18 - Data FIFO access window of endpoint or channel"]
    #[inline(always)]
    pub fn fifo_iter(&self) -> impl Iterator<Item = &Fifo> {
        (0..6)
            .map(move |n| unsafe { &*core::ptr::from_ref(self).cast::<u8>().add(4096 * n).cast() })
    }
}
#[doc = "FIFO (rw) register accessor: Data FIFO access window of endpoint or channel\n\nYou can [`read`](crate::Reg::read) this register and get [`fifo::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`fifo::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@fifo`] module"]
#[doc(alias = "FIFO")]
pub type Fifo = crate::Reg<fifo::FifoSpec>;
#[doc = "Data FIFO access window of endpoint or channel"]
pub mod fifo;
