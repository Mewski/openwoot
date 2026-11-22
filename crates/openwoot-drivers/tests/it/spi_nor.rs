use core::convert::Infallible;

use embassy_futures::block_on;
use embedded_hal_async::spi::{ErrorType, Operation, SpiDevice};
use embedded_storage_async::nor_flash::{NorFlash, ReadNorFlash};
use openwoot_drivers::spi_nor::{Error, GIGADEVICE, Identification, SpiNor, WINBOND};

const CAPACITY: u32 = 64 * 1024;
const BUSY_POLLS: u32 = 3;
const UNIQUE_ID: [u8; 8] = [0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87];

struct Chip {
    memory: Vec<u8>,
    write_enabled: bool,
    busy_polls: u32,
    commands: Vec<Vec<u8>>,
    status_reads: u32,
}

impl Chip {
    fn new() -> Self {
        Self {
            memory: vec![0xFF; CAPACITY as usize],
            write_enabled: false,
            busy_polls: 0,
            commands: Vec::new(),
            status_reads: 0,
        }
    }

    fn output(&mut self, sent: &[u8], already: usize, buffer: &mut [u8]) {
        match sent.first() {
            Some(0x05) => {
                assert_eq!(sent.len(), 1);
                self.status_reads += 1;
                let busy = self.busy_polls > 0;
                self.busy_polls = self.busy_polls.saturating_sub(1);
                buffer.fill(u8::from(busy));
            }
            Some(0x0B) => {
                assert_eq!(sent.len(), 5, "fast read needs address and one dummy byte");
                let start = address(sent) as usize + already;
                let cells = self.memory.iter().cycle().skip(start);
                for (byte, cell) in buffer.iter_mut().zip(cells) {
                    *byte = *cell;
                }
            }
            Some(0x90) => {
                assert_eq!(sent, [0x90, 0, 0, 0]);
                let ids = [WINBOND, 0x16].into_iter().cycle().skip(already);
                for (byte, id) in buffer.iter_mut().zip(ids) {
                    *byte = id;
                }
            }
            Some(0x4B) => {
                assert_eq!(sent, [0x4B, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
                for (byte, id) in buffer.iter_mut().zip(UNIQUE_ID.iter().skip(already)) {
                    *byte = *id;
                }
            }
            other => unreachable!("read phase after opcode {other:02x?}"),
        }
    }

    fn finish(&mut self, sent: Vec<u8>) {
        let opcode = sent.first().copied().unwrap_or_default();
        if opcode != 0x05 {
            assert_eq!(self.busy_polls, 0, "command {opcode:#04x} sent while busy");
        }
        match opcode {
            0x06 => self.write_enabled = true,
            0x02 => {
                assert!(self.write_enabled, "page program without write enable");
                let start = address(&sent) as usize;
                let base = start - start % 256;
                for (index, byte) in sent.iter().skip(4).enumerate() {
                    let cell = base + (start % 256 + index) % 256;
                    if let Some(cell) = self.memory.get_mut(cell) {
                        *cell &= byte;
                    }
                }
                self.write_enabled = false;
                self.busy_polls = BUSY_POLLS;
            }
            0x20 => {
                assert!(self.write_enabled, "sector erase without write enable");
                let start = address(&sent) as usize & !0xFFF;
                for cell in self.memory.iter_mut().skip(start).take(4096) {
                    *cell = 0xFF;
                }
                self.write_enabled = false;
                self.busy_polls = BUSY_POLLS;
            }
            _ => {}
        }
        self.commands.push(sent);
    }

    fn programs(&self) -> Vec<(u32, usize)> {
        self.commands
            .iter()
            .filter(|command| command.first() == Some(&0x02))
            .map(|command| (address(command), command.len() - 4))
            .collect()
    }
}

impl ErrorType for Chip {
    type Error = Infallible;
}

impl SpiDevice for Chip {
    async fn transaction(
        &mut self,
        operations: &mut [Operation<'_, u8>],
    ) -> Result<(), Infallible> {
        let mut sent = Vec::new();
        let mut read = 0;
        for operation in operations {
            match operation {
                Operation::Write(bytes) => sent.extend_from_slice(bytes),
                Operation::Read(buffer) => {
                    self.output(&sent, read, buffer);
                    read += buffer.len();
                }
                Operation::Transfer(..) | Operation::TransferInPlace(..) => {
                    unreachable!("the driver only writes and reads")
                }
                Operation::DelayNs(_) => {}
            }
        }
        self.finish(sent);
        embassy_futures::yield_now().await;
        Ok(())
    }
}

fn address(command: &[u8]) -> u32 {
    match command {
        [_, high, middle, low, ..] => u32::from_be_bytes([0, *high, *middle, *low]),
        _ => 0,
    }
}

#[test]
fn writes_split_at_page_boundaries_and_read_back() {
    let mut flash = SpiNor::new(Chip::new(), CAPACITY);
    let data: Vec<u8> = (0..600u32)
        .map(|index| u8::try_from(index * 7 % 256).unwrap())
        .collect();
    block_on(flash.write(0x1F0, &data)).unwrap();
    assert_eq!(
        flash.release().programs(),
        [(0x1F0, 16), (0x200, 256), (0x300, 256), (0x400, 72)]
    );
    let mut flash = SpiNor::new(Chip::new(), CAPACITY);
    block_on(flash.write(0x1F0, &data)).unwrap();
    let mut back = vec![0; data.len()];
    block_on(flash.read(0x1F0, &mut back)).unwrap();
    assert_eq!(back, data);
}

#[test]
fn every_program_and_erase_is_enabled_and_awaited() {
    let mut flash = SpiNor::new(Chip::new(), CAPACITY);
    block_on(flash.write(0x2000, &[0x12, 0x34, 0x56, 0x78])).unwrap();
    block_on(flash.erase(0x2000, 0x4000)).unwrap();
    let chip = flash.release();
    let opcodes: Vec<u8> = chip
        .commands
        .iter()
        .map(|command| command[0])
        .filter(|opcode| *opcode != 0x05)
        .collect();
    assert_eq!(opcodes, [0x06, 0x02, 0x06, 0x20, 0x06, 0x20]);
    assert_eq!(chip.busy_polls, 0);
    assert!(chip.status_reads >= 3 * (BUSY_POLLS + 1));
    assert!(chip.memory[0x2000..0x4000].iter().all(|byte| *byte == 0xFF));
}

#[test]
fn programming_only_clears_bits() {
    let mut flash = SpiNor::new(Chip::new(), CAPACITY);
    block_on(flash.write(0x100, &[0xF0, 0xFF, 0x0F, 0xFF])).unwrap();
    block_on(flash.write(0x100, &[0x3C, 0x00, 0xFF, 0xFF])).unwrap();
    let mut back = [0; 4];
    block_on(flash.read(0x100, &mut back)).unwrap();
    assert_eq!(back, [0x30, 0x00, 0x0F, 0xFF]);
}

#[test]
fn identification_and_unique_id_use_the_chip_command_bytes() {
    let mut flash = SpiNor::new(Chip::new(), CAPACITY);
    let id = block_on(flash.identification()).unwrap();
    assert_eq!(
        id,
        Identification {
            manufacturer: WINBOND,
            device: 0x16
        }
    );
    assert!(id.is_known());
    assert_ne!(id.manufacturer, GIGADEVICE);
    assert_eq!(block_on(flash.unique_id()).unwrap(), UNIQUE_ID);
}

#[test]
fn misaligned_and_outside_accesses_are_refused_without_bus_traffic() {
    let mut flash = SpiNor::new(Chip::new(), CAPACITY);
    assert_eq!(
        block_on(flash.write(0x101, &[0; 4])),
        Err(Error::NotAligned)
    );
    assert_eq!(
        block_on(flash.write(0x100, &[0; 3])),
        Err(Error::NotAligned)
    );
    assert_eq!(block_on(flash.erase(0x800, 0x1000)), Err(Error::NotAligned));
    assert_eq!(
        block_on(flash.erase(CAPACITY, CAPACITY + 0x1000)),
        Err(Error::OutOfBounds)
    );
    let mut buffer = [0; 8];
    assert_eq!(
        block_on(flash.read(CAPACITY - 4, &mut buffer)),
        Err(Error::OutOfBounds)
    );
    assert_eq!(flash.release().commands, Vec::<Vec<u8>>::new());
}
