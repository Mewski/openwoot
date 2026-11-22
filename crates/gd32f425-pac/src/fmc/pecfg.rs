#[doc = "Register `PECFG` reader"]
pub type R = crate::R<PecfgSpec>;
#[doc = "Register `PECFG` writer"]
pub type W = crate::W<PecfgSpec>;
#[doc = "Field `PE_ADDR` reader - Page erase address"]
pub type PeAddrR = crate::FieldReader<u32>;
#[doc = "Field `PE_ADDR` writer - Page erase address"]
pub type PeAddrW<'a, REG> = crate::FieldWriter<'a, REG, 29, u32>;
#[doc = "Field `PE_EN` reader - Page erase function enable"]
pub type PeEnR = crate::BitReader;
#[doc = "Field `PE_EN` writer - Page erase function enable"]
pub type PeEnW<'a, REG> = crate::BitWriter<'a, REG>;
impl R {
    #[doc = "Bits 0:28 - Page erase address"]
    #[inline(always)]
    pub fn pe_addr(&self) -> PeAddrR {
        PeAddrR::new(self.bits & 0x1fff_ffff)
    }

    #[doc = "Bit 31 - Page erase function enable"]
    #[inline(always)]
    pub fn pe_en(&self) -> PeEnR {
        PeEnR::new(((self.bits >> 31) & 1) != 0)
    }
}
impl W {
    #[doc = "Bits 0:28 - Page erase address"]
    #[inline(always)]
    pub fn pe_addr(&mut self) -> PeAddrW<'_, PecfgSpec> {
        PeAddrW::new(self, 0)
    }

    #[doc = "Bit 31 - Page erase function enable"]
    #[inline(always)]
    pub fn pe_en(&mut self) -> PeEnW<'_, PecfgSpec> {
        PeEnW::new(self, 31)
    }
}
#[doc = "Page erase configuration register\n\nYou can [`read`](crate::Reg::read) this register and get [`pecfg::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`pecfg::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct PecfgSpec;
impl crate::RegisterSpec for PecfgSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`pecfg::R`](R) reader structure"]
impl crate::Readable for PecfgSpec {}
#[doc = "`write(|w| ..)` method takes [`pecfg::W`](W) writer structure"]
impl crate::Writable for PecfgSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets PECFG to value 0"]
impl crate::Resettable for PecfgSpec {}
