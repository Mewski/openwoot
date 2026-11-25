use core::sync::atomic::{AtomicU8, Ordering};

use embassy_futures::select::select;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer};

const CLOSED: u8 = 0;
const WANTED: u8 = 1;
const OPEN: u8 = 2;

pub struct LedWindow {
    state: AtomicU8,
    opened: Signal<CriticalSectionRawMutex, ()>,
    closed: Signal<CriticalSectionRawMutex, ()>,
}

pub(crate) struct LedTransfer<'w>(&'w LedWindow);

impl Default for LedWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl LedWindow {
    pub const TRANSFER_LIMIT: Duration = Duration::from_millis(2);

    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(CLOSED),
            opened: Signal::new(),
            closed: Signal::new(),
        }
    }

    pub async fn grant(&self) {
        if self
            .state
            .compare_exchange(WANTED, OPEN, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.opened.signal(());
            select(self.closing(), Timer::after(Self::TRANSFER_LIMIT)).await;
        }
    }

    pub(crate) async fn enter(&self) -> LedTransfer<'_> {
        self.state.store(WANTED, Ordering::Release);
        let transfer = LedTransfer(self);
        while self.state.load(Ordering::Acquire) != OPEN {
            self.opened.wait().await;
        }
        transfer
    }

    async fn closing(&self) {
        while self.state.load(Ordering::Acquire) == OPEN {
            self.closed.wait().await;
        }
    }
}

impl Drop for LedTransfer<'_> {
    fn drop(&mut self) {
        self.0.state.store(CLOSED, Ordering::Release);
        self.0.closed.signal(());
    }
}

#[cfg(test)]
mod tests {
    use core::{
        pin::pin,
        task::{Context, Poll, Waker},
    };

    use super::*;

    #[test]
    fn a_grant_ends_at_once_while_no_transfer_waits() {
        let window = LedWindow::new();
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(pin!(window.grant()).poll(&mut context), Poll::Ready(()));
    }

    #[test]
    fn a_transfer_waits_for_the_grant_and_the_grant_for_the_end_of_the_transfer() {
        let window = LedWindow::new();
        let mut context = Context::from_waker(Waker::noop());
        let mut enter = pin!(window.enter());
        assert!(enter.as_mut().poll(&mut context).is_pending());
        assert!(enter.as_mut().poll(&mut context).is_pending());
        let mut grant = pin!(window.grant());
        assert!(grant.as_mut().poll(&mut context).is_pending());
        let Poll::Ready(transfer) = enter.as_mut().poll(&mut context) else {
            panic!("the granted transfer still waits");
        };
        assert!(grant.as_mut().poll(&mut context).is_pending());
        drop(transfer);
        assert_eq!(grant.as_mut().poll(&mut context), Poll::Ready(()));
        assert_eq!(
            pin!(window.grant()).poll(&mut context),
            Poll::Ready(()),
            "the next grant waits for a transfer that was never asked for"
        );
    }

    #[test]
    fn a_transfer_given_up_before_its_grant_leaves_nothing_to_wait_for() {
        let window = LedWindow::new();
        let mut context = Context::from_waker(Waker::noop());
        {
            let mut enter = pin!(window.enter());
            assert!(enter.as_mut().poll(&mut context).is_pending());
        }
        assert_eq!(pin!(window.grant()).poll(&mut context), Poll::Ready(()));
        let mut enter = pin!(window.enter());
        assert!(enter.as_mut().poll(&mut context).is_pending());
    }

    #[test]
    fn a_grant_left_early_lets_its_transfer_finish_without_granting_it_again() {
        let window = LedWindow::new();
        let mut context = Context::from_waker(Waker::noop());
        let mut enter = pin!(window.enter());
        assert!(enter.as_mut().poll(&mut context).is_pending());
        {
            let mut grant = pin!(window.grant());
            assert!(grant.as_mut().poll(&mut context).is_pending());
        }
        let Poll::Ready(transfer) = enter.as_mut().poll(&mut context) else {
            panic!("the granted transfer still waits");
        };
        assert_eq!(
            pin!(window.grant()).poll(&mut context),
            Poll::Ready(()),
            "a second grant waits for the transfer of the first"
        );
        drop(transfer);
        let mut next = pin!(window.enter());
        assert!(next.as_mut().poll(&mut context).is_pending());
        let mut grant = pin!(window.grant());
        assert!(grant.as_mut().poll(&mut context).is_pending());
        assert!(next.as_mut().poll(&mut context).is_ready());
    }
}
