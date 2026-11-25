use std::{cell::RefCell, collections::VecDeque, future::ready, rc::Rc, time::Duration};

use openwoot_host::{
    bootloader::{Port, REPLY_LEN, Request, SPLIT_REPORT_LEN},
    transfer,
};

const BLOCK_LEN: usize = 256;
const PAGE_BLOCKS: u32 = 16;
const APPLICATION_BLOCKS: u32 = 0xF60;
const UNLOCK_KEY: u32 = 0xFFAA_FFBB;
const EXECUTED: u8 = 0x88;
const REFUSED: u8 = 0xFF;
const UNKNOWN: u8 = 0x66;
const WRONG_MAGIC: u8 = 0x33;
const MAGIC: [u8; 2] = [0xDF, 0xDB];
const ECHO_ANY: u8 = 0xFF;

#[derive(Debug, Default)]
pub(super) struct Faults {
    pub silent_pings: usize,
    pub flipped_block: Option<u32>,
    pub wrong_echo_to_block: Option<u32>,
    pub disconnect_after_requests: Option<usize>,
    pub refused: Option<u8>,
    pub echo_any_to: Option<u8>,
    pub wrong_magic_to: Option<u8>,
    pub empty_reply_to: Option<u8>,
    pub short_crc: bool,
    pub repeated_reply_to: Option<u8>,
    pub last_block_off_by_one: bool,
    pub failed_restart: bool,
    pub report_lens_to: Option<(u8, Vec<usize>)>,
}

#[derive(Debug)]
pub(super) struct State {
    pub flash: Vec<u8>,
    pub marker: u32,
    pub flags: u8,
    pub version: [u8; 2],
    pub serial: Option<String>,
    pub report_len: usize,
    pub echoes_command: bool,
    pub outputs: Vec<usize>,
    pub commands: Vec<u8>,
    pub parameters: Vec<u32>,
    pub timeouts: Vec<Duration>,
    pub rebooted_with: Option<u32>,
    pub pauses: usize,
    pub faults: Faults,
    unlocked: bool,
    staging: Vec<u8>,
    current: Vec<u8>,
    last_block: u32,
    crc_end: usize,
    pending: VecDeque<(u8, Vec<u8>)>,
    disconnected: bool,
}

#[derive(Debug, Clone)]
pub(super) struct Bootloader(pub Rc<RefCell<State>>);

impl Bootloader {
    pub(super) fn new() -> Self {
        Self(Rc::new(RefCell::new(State {
            flash: vec![0xA5; APPLICATION_BLOCKS as usize * BLOCK_LEN],
            marker: 0,
            flags: 0x02,
            version: [2, 3],
            serial: Some("A1B2C3D4E5F6".to_owned()),
            report_len: REPLY_LEN,
            echoes_command: true,
            outputs: Vec::new(),
            commands: Vec::new(),
            parameters: Vec::new(),
            timeouts: Vec::new(),
            rebooted_with: None,
            pauses: 0,
            faults: Faults::default(),
            unlocked: false,
            staging: Vec::new(),
            current: vec![0; BLOCK_LEN],
            last_block: 0,
            crc_end: 0,
            pending: VecDeque::new(),
            disconnected: false,
        })))
    }

    pub(super) fn with_split_reports() -> Self {
        let bootloader = Self::new();
        {
            let mut state = bootloader.0.borrow_mut();
            state.version = [2, 0];
            state.serial = Some("A02B2437W08T00100S03H00250".to_owned());
            state.report_len = SPLIT_REPORT_LEN;
            state.echoes_command = false;
        }
        bootloader
    }
}

pub(super) fn crc24(bytes: &[u8]) -> u32 {
    let mut crc = 0;
    for &[low, high] in bytes.as_chunks::<2>().0 {
        let word = u32::from(u16::from_le_bytes([low, high]));
        let feedback = if crc & 0x80_0000 == 0 { 0 } else { 0x80_001B };
        crc = (((crc << 1) & 0xFF_FFFE) ^ word ^ feedback) & 0xFF_FFFF;
    }
    crc
}

impl State {
    fn connected(&self) -> Result<(), transfer::Error> {
        if self.disconnected {
            Err(transfer::Error::Disconnected)
        } else {
            Ok(())
        }
    }

    fn execute(&mut self, request: Request) -> Option<(u8, u8, Vec<u8>)> {
        let [first, second, command, p0, p1, p2, p3] = request;
        let parameter = u32::from_le_bytes([p0, p1, p2, p3]);
        if [first, second] != MAGIC {
            return Some((command, WRONG_MAGIC, Vec::new()));
        }
        self.commands.push(command);
        self.parameters.push(parameter);
        let needs_unlock = matches!(command, 2 | 3 | 8);
        if (needs_unlock && !self.unlocked) || self.faults.refused == Some(command) {
            return Some((command, REFUSED, Vec::new()));
        }
        let (status, body) = match command {
            0 => {
                if self.faults.silent_pings > 0 {
                    self.faults.silent_pings -= 1;
                    return None;
                }
                (EXECUTED, vec![!parameter.to_le_bytes()[0], self.flags])
            }
            1 => (EXECUTED, self.version.to_vec()),
            2 if (1..=0xFFFF).contains(&parameter) => {
                self.marker = u32::MAX;
                (EXECUTED, Vec::new())
            }
            3 if parameter < APPLICATION_BLOCKS => {
                self.write_block(parameter);
                if self.faults.wrong_echo_to_block == Some(parameter) {
                    return Some((4, EXECUTED, Vec::new()));
                }
                (EXECUTED, Vec::new())
            }
            4 => {
                let crc = self.flash.get(..self.crc_end).map_or(0, crc24);
                let mut body = crc.to_le_bytes().to_vec();
                if self.faults.short_crc {
                    body.truncate(2);
                }
                (EXECUTED, body)
            }
            5 => {
                if parameter & 1 != 0 {
                    self.marker = 0;
                }
                self.rebooted_with = Some(parameter);
                self.disconnected = true;
                return None;
            }
            6 if parameter == UNLOCK_KEY => {
                self.unlocked = true;
                (EXECUTED, Vec::new())
            }
            6 if self.unlocked => (EXECUTED, Vec::new()),
            7 => (EXECUTED, Vec::new()),
            8 => {
                let reported = self.last_block + u32::from(self.faults.last_block_off_by_one);
                (EXECUTED, reported.to_le_bytes().to_vec())
            }
            9 => match &self.serial {
                Some(serial) => (EXECUTED, serial.as_bytes().to_vec()),
                None => (REFUSED, Vec::new()),
            },
            10.. => (UNKNOWN, Vec::new()),
            _ => (REFUSED, Vec::new()),
        };
        Some((command, status, body))
    }

    fn write_block(&mut self, index: u32) {
        let start = index as usize * BLOCK_LEN;
        if index.is_multiple_of(PAGE_BLOCKS)
            && let Some(page) = self
                .flash
                .get_mut(start..start + PAGE_BLOCKS as usize * BLOCK_LEN)
        {
            page.fill(0xFF);
        }
        if self.faults.flipped_block == Some(index)
            && let Some(byte) = self.current.first_mut()
        {
            *byte ^= 0x01;
        }
        if let Some(cells) = self.flash.get_mut(start..start + BLOCK_LEN) {
            for (cell, byte) in cells.iter_mut().zip(&self.current) {
                *cell &= byte;
            }
        }
        self.current.fill(0xFF);
        self.last_block = index;
        self.crc_end = start + BLOCK_LEN;
    }

    fn queue(&mut self, command: u8, status: u8, body: &[u8]) {
        let Ok(length) = u8::try_from(body.len()) else {
            return;
        };
        if !self.pending.is_empty() {
            return;
        }
        let echo = if self.echoes_command && self.faults.echo_any_to != Some(command) {
            command
        } else {
            ECHO_ANY
        };
        let second = if self.faults.wrong_magic_to == Some(command) {
            MAGIC[1] ^ 0x01
        } else {
            MAGIC[1]
        };
        let header = [MAGIC[0], second, echo, status, length];
        let lens = match &self.faults.report_lens_to {
            Some((faulty, lens)) if *faulty == command => lens.clone(),
            _ => vec![self.report_len; REPLY_LEN / self.report_len],
        };
        let copies = if self.faults.repeated_reply_to == Some(command) {
            self.faults.repeated_reply_to = None;
            2
        } else {
            1
        };
        for _ in 0..copies {
            let mut reply = header
                .iter()
                .chain(body)
                .copied()
                .chain(std::iter::repeat(0));
            for &len in &lens {
                self.pending
                    .push_back((command, reply.by_ref().take(len).collect()));
            }
        }
    }

    fn accept_request(&mut self, request: Request) -> Result<(), transfer::Error> {
        self.connected()?;
        if let Some(limit) = &mut self.faults.disconnect_after_requests {
            if *limit == 0 {
                self.disconnected = true;
                return Err(transfer::Error::Disconnected);
            }
            *limit -= 1;
        }
        let [_, _, command, ..] = request;
        if let Some((echo, status, body)) = self.execute(request) {
            self.queue(echo, status, &body);
        }
        if command == 5 && self.faults.failed_restart {
            return Err(transfer::Error::Failed("broken pipe".to_owned()));
        }
        Ok(())
    }

    fn accept_report(&mut self, report: &[u8]) -> Result<(), transfer::Error> {
        self.connected()?;
        if report.len() != self.report_len {
            return Err(transfer::Error::Failed(format!(
                "an output report of {} bytes",
                report.len()
            )));
        }
        self.outputs.push(report.len());
        for &byte in report {
            self.staging.push(byte);
            if self.staging.len() == BLOCK_LEN {
                self.current = std::mem::take(&mut self.staging);
            }
        }
        Ok(())
    }

    fn take_report(
        &mut self,
        report: &mut [u8],
        timeout: Duration,
    ) -> Result<Option<usize>, transfer::Error> {
        self.connected()?;
        self.timeouts.push(timeout);
        let Some((command, pending)) = self.pending.pop_front() else {
            return Ok(None);
        };
        if self.faults.empty_reply_to == Some(command) {
            self.pending.clear();
            return Ok(Some(0));
        }
        for (cell, byte) in report.iter_mut().zip(&pending) {
            *cell = *byte;
        }
        Ok(Some(pending.len()))
    }
}

impl Port for Bootloader {
    fn discard_replies(&mut self) -> impl Future<Output = Result<(), transfer::Error>> {
        let mut state = self.0.borrow_mut();
        state.pending.clear();
        ready(state.connected())
    }

    fn send_request(
        &mut self,
        request: &Request,
    ) -> impl Future<Output = Result<(), transfer::Error>> {
        ready(self.0.borrow_mut().accept_request(*request))
    }

    fn send_report(&mut self, report: &[u8]) -> impl Future<Output = Result<(), transfer::Error>> {
        ready(self.0.borrow_mut().accept_report(report))
    }

    fn receive_report(
        &mut self,
        report: &mut [u8],
        timeout: Duration,
    ) -> impl Future<Output = Result<Option<usize>, transfer::Error>> {
        ready(self.0.borrow_mut().take_report(report, timeout))
    }

    fn pause(&mut self, _duration: Duration) -> impl Future<Output = ()> {
        self.0.borrow_mut().pauses += 1;
        ready(())
    }
}
