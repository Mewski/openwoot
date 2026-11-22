#[doc = "Register `GSNPSID` reader"]
pub type R = crate::R<GsnpsidSpec>;
#[doc = "Field `ID` reader - Core identification"]
pub type IdR = crate::FieldReader<u32>;
impl R {
    #[doc = "Bits 0:31 - Core identification"]
    #[inline(always)]
    pub fn id(&self) -> IdR {
        IdR::new(self.bits)
    }
}
#[doc = "Core identification register\n\nYou can [`read`](crate::Reg::read) this register and get [`gsnpsid::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct GsnpsidSpec;
impl crate::RegisterSpec for GsnpsidSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`gsnpsid::R`](R) reader structure"]
impl crate::Readable for GsnpsidSpec {}
#[doc = "`reset()` method sets GSNPSID to value 0"]
impl crate::Resettable for GsnpsidSpec {}
