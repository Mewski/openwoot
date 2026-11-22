use core::{convert::Infallible, future::pending};

use embassy_executor::SendSpawner;

use crate::Keyboard;

pub async fn run<Board: Keyboard>(board: Board, keys: SendSpawner) -> Infallible {
    let _running = (board, keys);
    pending().await
}
