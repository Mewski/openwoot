use core::{
    fmt,
    sync::atomic::{AtomicU8, Ordering},
};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal,
};
use openwoot_protocol::stock::{
    COMMAND_COUNT, Command, FIRST_CURRENT_COMMAND, Request, Status, WrongMagic,
};

const QUEUE_LEN: usize = 4;
const PENDING_COMMANDS: u8 = 3;

pub struct FeatureRequests {
    queue: Channel<CriticalSectionRawMutex, FeatureRequest, QUEUE_LEN>,
    pending_commands: AtomicU8,
    session_ends: Signal<CriticalSectionRawMutex, ()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeatureRequest {
    Command(Request),
    Status { command: u8, status: Status },
}

impl FeatureRequests {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queue: Channel::new(),
            pending_commands: AtomicU8::new(0),
            session_ends: Signal::new(),
        }
    }

    pub(super) fn submit(&self, data: &[u8]) -> bool {
        let request = match Request::parse(data) {
            Err(WrongMagic { command }) => FeatureRequest::Status {
                command,
                status: Status::Error,
            },
            Ok(request) if !accepts(request.command) => FeatureRequest::Status {
                command: request.command,
                status: Status::Unknown,
            },
            Ok(request) if self.pending_commands.load(Ordering::Relaxed) >= PENDING_COMMANDS => {
                FeatureRequest::Status {
                    command: request.command,
                    status: Status::Busy,
                }
            }
            Ok(request) => FeatureRequest::Command(request),
        };
        let queued = self.queue.try_send(request).is_ok();
        if queued && matches!(request, FeatureRequest::Command(_)) {
            self.pending_commands.fetch_add(1, Ordering::Relaxed);
        }
        queued
    }

    pub(super) fn end_session(&self) {
        while let Ok(request) = self.queue.try_receive() {
            self.release(request);
        }
        self.session_ends.signal(());
    }

    pub(crate) async fn receive(&self) -> FeatureRequest {
        let request = self.queue.receive().await;
        self.release(request);
        request
    }

    pub(crate) async fn session_ended(&self) {
        self.session_ends.wait().await;
    }

    fn release(&self, request: FeatureRequest) {
        if matches!(request, FeatureRequest::Command(_)) {
            self.pending_commands.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

impl Default for FeatureRequests {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for FeatureRequests {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FeatureRequests")
            .field("queue", &self.queue)
            .field("pending_commands", &self.pending_commands)
            .finish_non_exhaustive()
    }
}

const fn accepts(command: u8) -> bool {
    command < COMMAND_COUNT
        && (command >= FIRST_CURRENT_COMMAND || Command::from_number(command).is_some())
}

#[cfg(test)]
mod tests {
    use embassy_futures::block_on;

    use super::*;

    fn drain(requests: &FeatureRequests) -> [Option<FeatureRequest>; QUEUE_LEN] {
        core::array::from_fn(|_| requests.queue.try_receive().ok())
    }

    #[test]
    fn a_wrong_magic_and_an_unknown_command_are_answered_at_once_with_their_status() {
        let requests = FeatureRequests::new();
        requests.submit(&[0xD1, 0xDB, 1, 0, 0, 0, 0]);
        requests.submit(&[0xD1, 0xDA, 4, 0, 0, 0, 0]);
        requests.submit(&[0xD1, 0xDA, 89, 0, 0, 0, 0]);
        requests.submit(&[0xD1, 0xDA, 38, 0, 0, 0, 0]);
        assert_eq!(
            drain(&requests),
            [
                Some(FeatureRequest::Status {
                    command: 1,
                    status: Status::Error
                }),
                Some(FeatureRequest::Status {
                    command: 4,
                    status: Status::Unknown
                }),
                Some(FeatureRequest::Status {
                    command: 89,
                    status: Status::Unknown
                }),
                Some(FeatureRequest::Command(Request {
                    command: 38,
                    argument: 0
                })),
            ]
        );
    }

    #[test]
    fn a_fourth_waiting_command_is_answered_busy() {
        let requests = FeatureRequests::new();
        for command in [0, 1, 3, 19] {
            requests.submit(&[0xD1, 0xDA, command, 0, 0, 0, 0]);
        }
        assert_eq!(
            block_on(requests.receive()),
            FeatureRequest::Command(Request {
                command: 0,
                argument: 0
            })
        );
        assert_eq!(requests.pending_commands.load(Ordering::Relaxed), 2);
        assert_eq!(
            drain(&requests),
            [
                Some(FeatureRequest::Command(Request {
                    command: 1,
                    argument: 0
                })),
                Some(FeatureRequest::Command(Request {
                    command: 3,
                    argument: 0
                })),
                Some(FeatureRequest::Status {
                    command: 19,
                    status: Status::Busy
                }),
                None,
            ]
        );
    }

    #[test]
    fn a_request_arriving_at_a_full_queue_is_refused() {
        let requests = FeatureRequests::new();
        let queued: [bool; 6] = core::array::from_fn(|argument| {
            requests.submit(&[0xD1, 0xDA, 0, u8::try_from(argument).unwrap(), 0, 0, 0])
        });
        assert_eq!(queued, [true, true, true, true, false, false]);
        assert_eq!(requests.pending_commands.load(Ordering::Relaxed), 3);
        assert_eq!(
            drain(&requests),
            [
                Some(FeatureRequest::Command(Request {
                    command: 0,
                    argument: 0
                })),
                Some(FeatureRequest::Command(Request {
                    command: 0,
                    argument: 1
                })),
                Some(FeatureRequest::Command(Request {
                    command: 0,
                    argument: 2
                })),
                Some(FeatureRequest::Status {
                    command: 0,
                    status: Status::Busy
                }),
            ]
        );
    }

    #[test]
    fn a_session_end_drops_the_waiting_requests_and_frees_their_slots() {
        let requests = FeatureRequests::new();
        for command in [0, 2, 3, 19] {
            requests.submit(&[0xD1, 0xDA, command, 0, 0, 0, 0]);
        }
        requests.end_session();
        block_on(requests.session_ended());
        assert_eq!(requests.pending_commands.load(Ordering::Relaxed), 0);
        assert_eq!(drain(&requests), [None; QUEUE_LEN]);
        requests.submit(&[0xD1, 0xDA, 1, 0, 0, 0, 0]);
        assert_eq!(
            block_on(requests.receive()),
            FeatureRequest::Command(Request {
                command: 1,
                argument: 0
            })
        );
    }
}
