use core::ops::{BitAnd, BitOr, Not};

use crate::pac::generic::{Readable, Reg, RegisterSpec, Resettable, Writable};

pub(crate) fn modify_bits<Spec>(register: &Reg<Spec>, change: impl FnOnce(Spec::Ux) -> Spec::Ux)
where
    Spec: RegisterSpec + Readable + Writable,
{
    // SAFETY: every caller changes only bits that the reference manual defines for this register;
    // the other bits keep the value just read.
    register.modify(|read, write| unsafe { write.bits(change(read.bits())) });
}

pub(crate) fn set_bits<Spec>(register: &Reg<Spec>, mask: Spec::Ux)
where
    Spec: RegisterSpec + Readable + Writable,
    Spec::Ux: BitOr<Output = Spec::Ux>,
{
    modify_bits(register, |bits| bits | mask);
}

pub(crate) fn clear_bits<Spec>(register: &Reg<Spec>, mask: Spec::Ux)
where
    Spec: RegisterSpec + Readable + Writable,
    Spec::Ux: BitAnd<Output = Spec::Ux> + Not<Output = Spec::Ux>,
{
    modify_bits(register, |bits| bits & !mask);
}

pub(crate) fn write_bits<Spec>(register: &Reg<Spec>, value: Spec::Ux)
where
    Spec: RegisterSpec + Resettable + Writable,
{
    // SAFETY: every caller writes a value whose fields the reference manual defines for this
    // register.
    register.write(|write| unsafe { write.bits(value) });
}

pub(crate) fn read_bits<Spec>(register: &Reg<Spec>) -> Spec::Ux
where
    Spec: RegisterSpec + Readable,
{
    register.read().bits()
}
