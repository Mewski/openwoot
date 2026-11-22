#[doc = "Register `FIFO[%s]` reader"]
pub type R = crate::R<FifoSpec>;
#[doc = "Register `FIFO[%s]` writer"]
pub type W = crate::W<FifoSpec>;
#[doc = "Field `FIFO` reader - Data FIFO access window of endpoint or channel"]
pub type FifoR = crate::FieldReader<u32>;
#[doc = "Field `FIFO` writer - Data FIFO access window of endpoint or channel"]
pub type FifoW<'a, REG> = crate::FieldWriter<'a, REG, 32, u32>;
impl R {
    #[doc = "Bits 0:31 - Data FIFO access window of endpoint or channel"]
    #[inline(always)]
    pub fn fifo(&self) -> FifoR {
        FifoR::new(self.bits)
    }
}
impl W {
    #[doc = "Bits 0:31 - Data FIFO access window of endpoint or channel"]
    #[inline(always)]
    pub fn fifo(&mut self) -> FifoW<'_, FifoSpec> {
        FifoW::new(self, 0)
    }
}
#[doc = "Data FIFO access window of endpoint or channel\n\nYou can [`read`](crate::Reg::read) this register and get [`fifo::R`](R). You can [`reset`](crate::Reg::reset), [`write`](crate::Reg::write), [`write_with_zero`](crate::Reg::write_with_zero) this register using [`fifo::W`](W). You can also [`modify`](crate::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct FifoSpec;
impl crate::RegisterSpec for FifoSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`fifo::R`](R) reader structure"]
impl crate::Readable for FifoSpec {}
#[doc = "`write(|w| ..)` method takes [`fifo::W`](W) writer structure"]
impl crate::Writable for FifoSpec {
    type Safety = crate::Unsafe;
}
#[doc = "`reset()` method sets FIFO[%s] to value 0"]
impl crate::Resettable for FifoSpec {}
