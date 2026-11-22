use std::{cell::RefCell, collections::VecDeque, future::ready, rc::Rc, time::Duration};

use openwoot_cli::transfer::{self, Reports};
use openwoot_protocol::{
    ERROR_KEY, Endpoint, VarKey, Version, WireError, decode_message,
    diagnostics::Report,
    encode_message,
    endpoints::{EnterUpdateMode, GetDiagnostics, GetVersion},
    report,
    stock::{Command, Request, Status, encode_reply},
};

#[derive(Debug, Default)]
pub(super) struct State {
    pub openwoot: Option<Version>,
    pub stock_version: Option<[u8; 3]>,
    pub update_mode_requests: usize,
    pub feature_commands: Vec<(u8, u32)>,
    pub diagnostics: Option<Report>,
    pending: VecDeque<Vec<u8>>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Keyboard(pub Rc<RefCell<State>>);

impl Keyboard {
    pub(super) fn openwoot(version: Version) -> Self {
        Self(Rc::new(RefCell::new(State {
            openwoot: Some(version),
            stock_version: Some([2, 1, 2]),
            ..State::default()
        })))
    }

    pub(super) fn stock(version: [u8; 3]) -> Self {
        Self(Rc::new(RefCell::new(State {
            stock_version: Some(version),
            ..State::default()
        })))
    }
}

impl State {
    fn answer_frame(&mut self, report: &[u8]) {
        let Some(version) = self.openwoot else {
            return;
        };
        let Ok(Some(message)) = decode_message(report) else {
            return;
        };
        let mut reply = [0; report::MAX_LEN];
        let key = message.header.key;
        let encoded = if key == VarKey::Key8(GetVersion::REQ_KEY) {
            encode_message(
                &message.reply_header(GetVersion::RESP_KEY),
                &version,
                &mut reply,
            )
        } else if key == VarKey::Key8(EnterUpdateMode::REQ_KEY) {
            self.update_mode_requests += 1;
            encode_message(
                &message.reply_header(EnterUpdateMode::RESP_KEY),
                &(),
                &mut reply,
            )
        } else if let Some(diagnostics) = self
            .diagnostics
            .filter(|_| key == VarKey::Key8(GetDiagnostics::REQ_KEY))
        {
            encode_message(
                &message.reply_header(GetDiagnostics::RESP_KEY),
                &diagnostics,
                &mut reply,
            )
        } else {
            encode_message(
                &message.reply_header(ERROR_KEY),
                &WireError::UnknownKey,
                &mut reply,
            )
        };
        if let Ok(encoded) = encoded {
            self.pending.push_back(encoded.to_vec());
        }
    }

    fn answer_feature(&mut self, report: &[u8]) {
        let Some(Ok(request)) = report.get(1..).map(Request::parse) else {
            return;
        };
        self.feature_commands
            .push((request.command, request.argument));
        let Some(version) = self.stock_version else {
            return;
        };
        let mut reply = [0; report::MAX_LEN];
        let encoded = match Command::from_number(request.command) {
            Some(Command::Version) => {
                encode_reply(request.command, Status::Success, &version, &mut reply)
            }
            Some(Command::EnterUpdateMode) => return,
            _ => encode_reply(request.command, Status::Unknown, &[], &mut reply),
        };
        if let Ok(encoded) = encoded {
            self.pending.push_back(encoded.to_vec());
        }
    }
}

impl Reports for Keyboard {
    fn write_output(&mut self, report: &[u8]) -> impl Future<Output = Result<(), transfer::Error>> {
        self.0.borrow_mut().answer_frame(report);
        ready(Ok(()))
    }

    fn write_feature(
        &mut self,
        report: &[u8],
    ) -> impl Future<Output = Result<(), transfer::Error>> {
        self.0.borrow_mut().answer_feature(report);
        ready(Ok(()))
    }

    fn read_input(
        &mut self,
        report: &mut [u8],
        _timeout: Duration,
    ) -> impl Future<Output = Result<Option<usize>, transfer::Error>> {
        let reply = self.0.borrow_mut().pending.pop_front();
        let received = reply.and_then(|reply| {
            report.get_mut(..reply.len())?.copy_from_slice(&reply);
            Some(reply.len())
        });
        ready(Ok(received))
    }

    fn discard_input(&mut self) -> impl Future<Output = Result<(), transfer::Error>> {
        self.0.borrow_mut().pending.clear();
        ready(Ok(()))
    }
}
