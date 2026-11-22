use std::{future::ready, time::Duration};

use openwoot_cli::bootloader::{Port, PortError, REPLY_LEN, Reply, Request, crc24};

const BLOCK_LEN: usize = 256;
const PAGE_BLOCKS: u32 = 16;
const APPLICATION_BLOCKS: u32 = 0xF60;
const UNLOCK_KEY: u32 = 0xFFAA_FFBB;
const EXECUTED: u8 = 0x88;
const REFUSED: u8 = 0xFF;
const UNKNOWN: u8 = 0x66;
const WRONG_MAGIC: u8 = 0x33;
const MAGIC: [u8; 2] = [0xDF, 0xDB];

#[derive(Debug, Default)]
pub(super) struct Faults {
    pub silent_pings: usize,
    pub flipped_block: Option<u32>,
    pub wrong_echo_to_block: Option<u32>,
    pub disconnect_after_requests: Option<usize>,
}

#[derive(Debug)]
pub(super) struct Bootloader {
    pub flash: Vec<u8>,
    pub marker: u32,
    pub flags: u8,
    pub serial: Option<String>,
    pub commands: Vec<u8>,
    pub rebooted_with: Option<u32>,
    pub pauses: usize,
    pub faults: Faults,
    unlocked: bool,
    staging: Vec<u8>,
    current: Vec<u8>,
    last_block: u32,
    crc_end: usize,
    pending: Option<Reply>,
    disconnected: bool,
}

impl Bootloader {
    pub(super) fn new() -> Self {
        Self {
            flash: vec![0xA5; APPLICATION_BLOCKS as usize * BLOCK_LEN],
            marker: 0,
            flags: 0x02,
            serial: Some("A1B2C3D4E5F6".to_owned()),
            commands: Vec::new(),
            rebooted_with: None,
            pauses: 0,
            faults: Faults::default(),
            unlocked: false,
            staging: Vec::new(),
            current: vec![0; BLOCK_LEN],
            last_block: 0,
            crc_end: 0,
            pending: None,
            disconnected: false,
        }
    }

    fn connected(&self) -> Result<(), PortError> {
        if self.disconnected {
            Err(PortError::Disconnected)
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
        let needs_unlock = matches!(command, 2 | 3 | 8);
        if needs_unlock && !self.unlocked {
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
            1 => (EXECUTED, vec![2, 3]),
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
                (EXECUTED, crc.to_le_bytes().to_vec())
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
            8 => (EXECUTED, self.last_block.to_le_bytes().to_vec()),
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
        if self.pending.is_some() {
            return;
        }
        let header = [MAGIC[0], MAGIC[1], command, status, length];
        let mut reply = [0; REPLY_LEN];
        for (cell, byte) in reply.iter_mut().zip(header.iter().chain(body)) {
            *cell = *byte;
        }
        self.pending = Some(reply);
    }

    fn accept_request(&mut self, request: Request) -> Result<(), PortError> {
        self.connected()?;
        if let Some(limit) = &mut self.faults.disconnect_after_requests {
            if *limit == 0 {
                self.disconnected = true;
                return Err(PortError::Disconnected);
            }
            *limit -= 1;
        }
        if let Some((command, status, body)) = self.execute(request) {
            self.queue(command, status, &body);
        }
        Ok(())
    }

    fn accept_block(&mut self, block: &[u8]) -> Result<(), PortError> {
        self.connected()?;
        for &byte in block {
            self.staging.push(byte);
            if self.staging.len() == BLOCK_LEN {
                self.current = std::mem::take(&mut self.staging);
            }
        }
        Ok(())
    }

    fn take_reply(&mut self, reply: &mut Reply) -> Result<Option<usize>, PortError> {
        self.connected()?;
        Ok(self.pending.take().map(|pending| {
            *reply = pending;
            REPLY_LEN
        }))
    }
}

impl Port for Bootloader {
    fn discard_replies(&mut self) -> impl Future<Output = Result<(), PortError>> {
        self.pending = None;
        ready(self.connected())
    }

    fn send_request(&mut self, request: &Request) -> impl Future<Output = Result<(), PortError>> {
        ready(self.accept_request(*request))
    }

    fn send_block(&mut self, block: &[u8]) -> impl Future<Output = Result<(), PortError>> {
        ready(self.accept_block(block))
    }

    fn receive_reply(
        &mut self,
        reply: &mut Reply,
        _timeout: Duration,
    ) -> impl Future<Output = Result<Option<usize>, PortError>> {
        ready(self.take_reply(reply))
    }

    fn pause(&mut self, _duration: Duration) -> impl Future<Output = ()> {
        self.pauses += 1;
        ready(())
    }
}
