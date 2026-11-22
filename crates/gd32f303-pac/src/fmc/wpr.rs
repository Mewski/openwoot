#[doc = "Register `WPR` reader"]
pub type R = crate::R<WprSpec>;
#[doc = "Field `WP` reader - Store WP\\[31:0\\] of option bytes block after system reset"]
pub type WpR = crate::FieldReader<u32>;
impl R {
    #[doc = "Bits 0:31 - Store WP\\[31:0\\] of option bytes block after system reset"]
    #[inline(always)]
    pub fn wp(&self) -> WpR {
        WpR::new(self.bits)
    }
}
#[doc = "Erase/Program Protection register\n\nYou can [`read`](crate::Reg::read) this register and get [`wpr::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct WprSpec;
impl crate::RegisterSpec for WprSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`wpr::R`](R) reader structure"]
impl crate::Readable for WprSpec {}
#[doc = "`reset()` method sets WPR to value 0"]
impl crate::Resettable for WprSpec {}
