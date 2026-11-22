#[repr(C)]
#[doc = "Register block"]
pub struct RegisterBlock {
    data: (),
}
impl RegisterBlock {
    #[doc = "0x00..0x200 - Packet buffer half word"]
    #[inline(always)]
    pub const fn data(&self, n: usize) -> &Data {
        #[allow(clippy::no_effect)]
        [(); 256][n];
        unsafe { &*core::ptr::from_ref(self).cast::<u8>().add(4 * n).cast() }
    }

    #[doc = "Iterator for array of:"]
    #[doc = "0x00..0x200 - Packet buffer half word"]
    #[inline(always)]
    pub fn data_iter(&self) -> impl Iterator<Item = &Data> {
        (0..256).map(move |n| unsafe { &*core::ptr::from_ref(self).cast::<u8>().add(4 * n).cast() })
    }
}
#[doc = "DATA (rw) register accessor: Packet buffer half word\n\nYou can [`read`](crate::Reg::read) this register and get [`data::R`]. You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`data::W`]. You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@data`] module"]
#[doc(alias = "DATA")]
pub type Data = crate::Reg<data::DataSpec>;
#[doc = "Packet buffer half word"]
pub mod data;
