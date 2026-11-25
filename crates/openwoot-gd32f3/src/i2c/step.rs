use core::future::Future;

use embassy_time::{Duration, with_timeout};

use super::Error;

pub(super) const STEP_TIMEOUT: Duration = Duration::from_millis(10);

pub(super) async fn step<T>(wait: impl Future<Output = Result<T, Error>>) -> Result<T, Error> {
    with_timeout(STEP_TIMEOUT, wait)
        .await
        .unwrap_or(Err(Error::Timeout))
}

#[cfg(test)]
mod tests {
    use core::{
        cell::Cell,
        future::poll_fn,
        pin::{Pin, pin},
        task::{Context, Poll, Waker},
    };
    use std::sync::{Mutex, MutexGuard};

    use embassy_time::MockDriver;

    use super::*;

    static CLOCK: Mutex<()> = Mutex::new(());

    fn clock() -> (MutexGuard<'static, ()>, &'static MockDriver) {
        let guard = CLOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (guard, MockDriver::get())
    }

    fn flag(set: &Cell<bool>) -> impl Future<Output = Result<(), Error>> + '_ {
        poll_fn(move |_| {
            if set.get() {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
    }

    fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    #[test]
    fn a_step_the_bus_finished_while_its_task_waited_longer_than_the_time_out_succeeds() {
        let (_guard, clock) = clock();
        let finished = Cell::new(false);
        let mut waiting = pin!(step(flag(&finished)));
        assert_eq!(poll(waiting.as_mut()), Poll::Pending);
        finished.set(true);
        clock.advance(STEP_TIMEOUT * 3);
        assert_eq!(poll(waiting.as_mut()), Poll::Ready(Ok(())));
    }

    #[test]
    fn a_step_the_bus_does_not_finish_fails_once_its_time_out_has_passed() {
        let (_guard, clock) = clock();
        let finished = Cell::new(false);
        let mut waiting = pin!(step(flag(&finished)));
        assert_eq!(poll(waiting.as_mut()), Poll::Pending);
        clock.advance(STEP_TIMEOUT - Duration::from_ticks(1));
        assert_eq!(poll(waiting.as_mut()), Poll::Pending);
        clock.advance(Duration::from_ticks(1));
        assert_eq!(poll(waiting.as_mut()), Poll::Ready(Err(Error::Timeout)));
    }

    #[test]
    fn late_polls_between_finished_steps_add_up_to_no_time_out() {
        let (_guard, clock) = clock();
        let flags = [Cell::new(false), Cell::new(false), Cell::new(false)];
        let mut transfer = pin!(async {
            for finished in &flags {
                step(flag(finished)).await?;
            }
            Ok::<_, Error>(())
        });
        assert_eq!(poll(transfer.as_mut()), Poll::Pending);
        for (index, finished) in flags.iter().enumerate() {
            finished.set(true);
            clock.advance(STEP_TIMEOUT * 6 / 10);
            let expected = if index + 1 == flags.len() {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            };
            assert_eq!(poll(transfer.as_mut()), expected, "after step {index}");
        }
    }
}
