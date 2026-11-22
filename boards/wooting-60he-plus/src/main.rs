#![no_std]
#![no_main]

use embassy_executor::SendSpawner;
use wooting_60he_plus::{Board, DESCRIPTOR, entry, hal};

hal::app_descriptor!(DESCRIPTOR);

#[entry]
fn main() -> ! {
    let Some(peripherals) = hal::pac::Peripherals::take() else {
        hal::system::reset()
    };
    let board = Board::new(peripherals);
    hal::executor::run(|spawner, keys| {
        if let Ok(token) = firmware(board, keys) {
            spawner.spawn(token);
        }
    })
}

#[embassy_executor::task]
async fn firmware(board: Board, keys: SendSpawner) {
    match openwoot::runtime::run(board, keys).await {}
}

#[panic_handler]
fn halt(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
