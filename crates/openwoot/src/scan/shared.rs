use core::{
    future::poll_fn,
    sync::atomic::{AtomicBool, Ordering},
    task::Poll,
};

use atomic_waker::AtomicWaker;
use embassy_futures::yield_now;
use embassy_time::Instant;
use spin::mutex::SpinMutex;

use super::{Converter, Outcome, Scanner, Sensing, Travel};

pub struct Shared<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> {
    scanner: SpinMutex<Option<Scanner<S, INPUTS, ADDRESSES>>>,
    running: AtomicBool,
    waker: AtomicWaker,
    travel: Travel<INPUTS, ADDRESSES>,
}

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> Default
    for Shared<S, INPUTS, ADDRESSES>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> Shared<S, INPUTS, ADDRESSES> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scanner: SpinMutex::new(None),
            running: AtomicBool::new(false),
            waker: AtomicWaker::new(),
            travel: Travel::new(),
        }
    }

    #[must_use]
    pub fn install(&self, scanner: Scanner<S, INPUTS, ADDRESSES>) -> bool {
        if self.is_running() {
            return false;
        }
        self.scanner.try_lock().is_some_and(|mut slot| {
            *slot = Some(scanner);
            true
        })
    }

    #[must_use]
    pub const fn travel(&self) -> &Travel<INPUTS, ADDRESSES> {
        &self.travel
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub fn with<R>(
        &self,
        change: impl FnOnce(&mut Scanner<S, INPUTS, ADDRESSES>) -> R,
    ) -> Option<R> {
        if self.is_running() {
            return None;
        }
        let mut slot = self.scanner.try_lock()?;
        slot.as_mut().map(change)
    }

    #[must_use]
    pub fn start(&self) -> bool {
        if self.is_running() {
            return false;
        }
        {
            let Some(slot) = self.scanner.try_lock() else {
                return false;
            };
            if !slot.as_ref().is_some_and(Scanner::has_trigger) {
                return false;
            }
            self.running.store(true, Ordering::Release);
        }
        S::Converter::request_interrupt();
        true
    }

    pub async fn finished(&self) {
        poll_fn(|context| {
            self.waker.register(context.waker());
            if self.is_running() {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
    }

    pub async fn calibrate(&self) -> bool {
        if self.with(Scanner::start_sweep).is_none() || !self.start() {
            return false;
        }
        self.finished().await;
        true
    }

    pub async fn cycle(&self, now_ms: u32) -> bool {
        if self.with(|scanner| scanner.prepare(now_ms)).is_none() || !self.start() {
            return false;
        }
        self.finished().await;
        true
    }

    pub fn recover(&self) -> bool {
        self.running.store(false, Ordering::Release);
        self.with(|scanner| scanner.converter_mut().recover())
            .is_some()
    }

    pub async fn run(
        &self,
        mut between_cycles: impl FnMut(&mut Scanner<S, INPUTS, ADDRESSES>),
    ) -> ! {
        while !self.calibrate().await {
            yield_now().await;
        }
        loop {
            if self.with(&mut between_cycles).is_none() || !self.cycle(now_ms()).await {
                yield_now().await;
            }
        }
    }

    #[inline(always)]
    pub fn on_conversion(&self) {
        S::Converter::acknowledge();
        let outcome = match self.scanner.try_lock() {
            Some(mut slot) => match slot.as_mut() {
                Some(scanner) => scanner.on_conversion(&self.travel),
                None => Outcome::Finished,
            },
            None => return,
        };
        if outcome == Outcome::Finished {
            self.running.store(false, Ordering::Release);
            self.waker.wake();
        }
    }
}

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> core::fmt::Debug
    for Shared<S, INPUTS, ADDRESSES>
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Shared")
            .field("running", &self.is_running())
            .finish_non_exhaustive()
    }
}

fn now_ms() -> u32 {
    Instant::now().as_millis() as u32
}
