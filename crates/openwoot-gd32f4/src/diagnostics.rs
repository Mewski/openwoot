use openwoot::diagnostics::Hardware;

use crate::{i2c, scan, spi, stack};

#[must_use]
pub fn hardware() -> Hardware {
    Hardware {
        scan_overruns: scan::overruns(),
        led_bus: i2c::transfers(),
        flash_bus: spi::transfers(),
        stack: stack::usage(),
    }
}
