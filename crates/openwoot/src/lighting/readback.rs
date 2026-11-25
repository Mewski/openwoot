use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Subject {
    Frame,
    Lightbar,
    State,
}

pub(super) struct Readback {
    request: Signal<CriticalSectionRawMutex, Subject>,
    published: Signal<CriticalSectionRawMutex, Subject>,
}

struct Withdrawal<'a>(&'a Signal<CriticalSectionRawMutex, Subject>);

impl Readback {
    pub(super) const fn new() -> Self {
        Self {
            request: Signal::new(),
            published: Signal::new(),
        }
    }

    pub(super) fn requested(&self) -> Option<Subject> {
        self.request.try_take()
    }

    pub(super) fn publish(&self, subject: Subject) {
        self.published.signal(subject);
    }

    pub(super) async fn read(&self, subject: Subject) {
        let _withdrawal = Withdrawal(&self.request);
        self.published.reset();
        self.request.signal(subject);
        while self.published.wait().await != subject {}
    }
}

impl Drop for Withdrawal<'_> {
    fn drop(&mut self) {
        self.0.reset();
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
    fn a_read_waits_for_the_publication_of_its_subject() {
        let readback = Readback::new();
        let mut read = pin!(readback.read(Subject::Lightbar));
        let mut context = Context::from_waker(Waker::noop());
        assert!(read.as_mut().poll(&mut context).is_pending());
        assert_eq!(readback.requested(), Some(Subject::Lightbar));
        readback.publish(Subject::Frame);
        assert!(read.as_mut().poll(&mut context).is_pending());
        readback.publish(Subject::Lightbar);
        assert_eq!(read.as_mut().poll(&mut context), Poll::Ready(()));
    }

    #[test]
    fn an_abandoned_read_leaves_no_request_behind() {
        let readback = Readback::new();
        {
            let mut read = pin!(readback.read(Subject::Frame));
            let mut context = Context::from_waker(Waker::noop());
            assert!(read.as_mut().poll(&mut context).is_pending());
        }
        assert_eq!(readback.requested(), None);
    }
}
