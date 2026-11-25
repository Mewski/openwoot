use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Subject {
    Frame,
    Lightbar,
    State,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LedState {
    pub(crate) host_control: bool,
    pub(crate) idle_off: bool,
}

pub(super) struct Readback {
    request: Signal<CriticalSectionRawMutex, Subject>,
    published: Signal<CriticalSectionRawMutex, Publication>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Publication {
    subject: Subject,
    state: LedState,
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

    pub(super) fn publish(&self, subject: Subject, state: LedState) {
        self.published.signal(Publication { subject, state });
    }

    pub(super) async fn read(&self, subject: Subject) -> LedState {
        let _withdrawal = Withdrawal(&self.request);
        self.published.reset();
        self.request.signal(subject);
        loop {
            let publication = self.published.wait().await;
            if publication.subject == subject {
                return publication.state;
            }
        }
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

    const IDLE_OFF: LedState = LedState {
        host_control: false,
        idle_off: true,
    };

    #[test]
    fn a_read_waits_for_the_publication_of_its_subject() {
        let readback = Readback::new();
        let mut read = pin!(readback.read(Subject::Lightbar));
        let mut context = Context::from_waker(Waker::noop());
        assert!(read.as_mut().poll(&mut context).is_pending());
        assert_eq!(readback.requested(), Some(Subject::Lightbar));
        readback.publish(Subject::Frame, LedState::default());
        assert!(read.as_mut().poll(&mut context).is_pending());
        readback.publish(Subject::Lightbar, IDLE_OFF);
        assert_eq!(read.as_mut().poll(&mut context), Poll::Ready(IDLE_OFF));
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
