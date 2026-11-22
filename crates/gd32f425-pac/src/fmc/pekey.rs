#[doc = "Register `PEKEY` writer"]
pub type W = crate::W<PekeySpec>;
#[doc = "Field `PE_KEY` writer - Page erase unlock key"]
pub type PeKeyW<'a, REG> = crate::FieldWriter<'a, REG, 32, u32>;
impl W {
    #[doc = "Bits 0:31 - Page erase unlock key"]
    #[inline(always)]
    pub fn pe_key(&mut self) -> PeKeyW<'_, PekeySpec> {
        PeKeyW::new(self, 0)
    }
}
#[doc = "Page erase unlock key register\n\nYou can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pekey::W`](W). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PekeySpec;
impl crate::RegisterSpec for PekeySpec {
    type Ux = u32;
}
#[doc = "`write(|w| ..)` method takes [`pekey::W`](W) writer structure"]
impl crate::Writable for PekeySpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PEKEY to value 0"]
impl crate::Resettable for PekeySpec {}
