#[doc = "Register `CS` reader"]
pub type R = crate::R<CsSpec>;
#[doc = "Register `CS` writer"]
pub type W = crate::W<CsSpec>;
#[doc = "Field `WUF` reader - Wakeup flag"]
pub type WufR = crate::BitReader;
#[doc = "Field `STBF` reader - Standby flag"]
pub type StbfR = crate::BitReader;
#[doc = "Field `LVDF` reader - Low Voltage Detector Status Flag"]
pub type LvdfR = crate::BitReader;
#[doc = "Field `WUPEN` reader - Enable WKUP pin"]
pub type WupenR = crate::BitReader;
#[doc = "Field `WUPEN` writer - Enable WKUP pin"]
pub type WupenW<'a, REG> = crate::BitWriter<'a, REG>;
#[doc = "Field `LDOVSRF` reader - LDO voltage select ready flag"]
pub type LdovsrfR = crate::BitReader;
#[doc = "Field `HDRF` reader - High-driver ready flag"]
pub type HdrfR = crate::BitReader;
#[doc = "Field `HDSRF` reader - High-driver switch ready flag"]
pub type HdsrfR = crate::BitReader;
#[doc = "Field `LDRF` reader - Low-driver mode ready flag"]
pub type LdrfR = crate::FieldReader;
#[doc = "Field `LDRF` writer - Low-driver mode ready flag"]
pub type LdrfW<'a, REG> = crate::FieldWriter<'a, REG, 2>;
impl R {
    #[doc = "Bit 0 - Wakeup flag"]
    #[inline(always)]
    pub fn wuf(&self) -> WufR {
        WufR::new((self.bits & 1) != 0)
    }

    #[doc = "Bit 1 - Standby flag"]
    #[inline(always)]
    pub fn stbf(&self) -> StbfR {
        StbfR::new(((self.bits >> 1) & 1) != 0)
    }

    #[doc = "Bit 2 - Low Voltage Detector Status Flag"]
    #[inline(always)]
    pub fn lvdf(&self) -> LvdfR {
        LvdfR::new(((self.bits >> 2) & 1) != 0)
    }

    #[doc = "Bit 8 - Enable WKUP pin"]
    #[inline(always)]
    pub fn wupen(&self) -> WupenR {
        WupenR::new(((self.bits >> 8) & 1) != 0)
    }

    #[doc = "Bit 14 - LDO voltage select ready flag"]
    #[inline(always)]
    pub fn ldovsrf(&self) -> LdovsrfR {
        LdovsrfR::new(((self.bits >> 14) & 1) != 0)
    }

    #[doc = "Bit 16 - High-driver ready flag"]
    #[inline(always)]
    pub fn hdrf(&self) -> HdrfR {
        HdrfR::new(((self.bits >> 16) & 1) != 0)
    }

    #[doc = "Bit 17 - High-driver switch ready flag"]
    #[inline(always)]
    pub fn hdsrf(&self) -> HdsrfR {
        HdsrfR::new(((self.bits >> 17) & 1) != 0)
    }

    #[doc = "Bits 18:19 - Low-driver mode ready flag"]
    #[inline(always)]
    pub fn ldrf(&self) -> LdrfR {
        LdrfR::new(((self.bits >> 18) & 3) as u8)
    }
}
impl W {
    #[doc = "Bit 8 - Enable WKUP pin"]
    #[inline(always)]
    pub fn wupen(&mut self) -> WupenW<'_, CsSpec> {
        WupenW::new(self, 8)
    }

    #[doc = "Bits 18:19 - Low-driver mode ready flag"]
    #[inline(always)]
    pub fn ldrf(&mut self) -> LdrfW<'_, CsSpec> {
        LdrfW::new(self, 18)
    }
}
#[doc = "power control/status register\n\nYou can [`read`](crate::Reg::read) this register and get [`cs::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`cs::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct CsSpec;
impl crate::RegisterSpec for CsSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`cs::R`](R) reader structure"]
impl crate::Readable for CsSpec {}
#[doc = "`write(|w| ..)` method takes [`cs::W`](W) writer structure"]
impl crate::Writable for CsSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets CS to value 0"]
impl crate::Resettable for CsSpec {}
