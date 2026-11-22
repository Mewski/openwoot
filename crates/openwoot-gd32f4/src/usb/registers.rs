use embassy_usb_synopsys_otg::otg_v1::Otg;

use crate::pac;

pub(super) fn registers() -> Otg {
    // SAFETY: `Driver::new` consumed the USBHS peripherals, so the driver and its interrupt handler
    // are the only users of the register block from then on.
    unsafe { Otg::from_ptr(pac::HsGlobal::PTR.cast_mut().cast()) }
}
