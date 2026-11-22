use core::sync::atomic::{AtomicU8, Ordering};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use openwoot_protocol::stock::{
    COMMAND_COUNT, Command, FIRST_CURRENT_COMMAND, Request, Status, WrongMagic,
};

const QUEUE_LEN: usize = 4;
const PENDING_COMMANDS: u8 = 3;

#[derive(Debug)]
pub struct FeatureRequests {
    queue: Channel<CriticalSectionRawMutex, FeatureRequest, QUEUE_LEN>,
    pending_commands: AtomicU8,
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
        }
    }

    pub(super) fn submit(&self, data: &[u8]) {
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
        if self.queue.try_send(request).is_ok() && matches!(request, FeatureRequest::Command(_)) {
            self.pending_commands.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) async fn receive(&self) -> FeatureRequest {
        let request = self.queue.receive().await;
        if matches!(request, FeatureRequest::Command(_)) {
            self.pending_commands.fetch_sub(1, Ordering::Relaxed);
        }
        request
    }
}

impl Default for FeatureRequests {
    fn default() -> Self {
        Self::new()
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
}
