#![no_std]
#![no_main]

use wooting_60he_plus::{Board, DESCRIPTOR, entry, hal};

hal::app_descriptor!(DESCRIPTOR);

#[entry]
fn main() -> ! {
    openwoot::runtime::run(Board::new())
}

#[panic_handler]
fn halt(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
