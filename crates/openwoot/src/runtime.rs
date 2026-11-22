use core::{convert::Infallible, future::pending};

use crate::Keyboard;

pub fn run<Board: Keyboard>(board: Board) -> ! {
    match embassy_futures::block_on(idle_task(board)) {}
}

async fn idle_task<Board: Keyboard>(board: Board) -> Infallible {
    let _board = board;
    pending().await
}
