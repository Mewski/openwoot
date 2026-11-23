pub(super) const ENDPOINT_COUNT: usize = ENDPOINTS as usize;
pub(super) const CONTROL_PACKET_LIMIT: u16 = 64;

const ENDPOINTS: u16 = 8;
const SIZE: u16 = 512;
const TABLE_ENTRY_LEN: u16 = 8;
const TABLE_LEN: u16 = TABLE_ENTRY_LEN * ENDPOINTS;
const CONTROL_RECEIVE: u16 = TABLE_LEN;
const CONTROL_TRANSMIT: u16 = CONTROL_RECEIVE + CONTROL_PACKET_LIMIT;
const FIRST_FREE: u16 = CONTROL_TRANSMIT + CONTROL_PACKET_LIMIT;
const HALFWORDS_PER_ENTRY: usize = 4;
const COUNT_MASK: u16 = 0x03FF;
const SMALL_BLOCK: u16 = 2;
const SMALL_BLOCK_LIMIT: u16 = 62;
const LARGE_BLOCK: u16 = 32;
const LARGE_BLOCK_FIRST: u16 = SMALL_BLOCK_LIMIT + 1;
const LARGE_BLOCK_LIMIT: u16 = 1024;
const LARGE_BLOCKS: u16 = 1 << 15;
const BLOCK_COUNT_SHIFT: u16 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Buffer {
    address: u16,
    len: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ReceiveBuffer {
    pub(super) buffer: Buffer,
    pub(super) count_field: u16,
}

#[derive(Debug)]
pub(super) struct Allocator {
    next: u16,
}

impl Allocator {
    pub(super) const fn new() -> Self {
        Self { next: FIRST_FREE }
    }

    pub(super) fn transmit(&mut self, max_packet_size: u16) -> Option<Buffer> {
        self.take(max_packet_size.checked_next_multiple_of(SMALL_BLOCK)?)
    }

    pub(super) fn receive(&mut self, max_packet_size: u16) -> Option<ReceiveBuffer> {
        let (len, count_field) = receive_capacity(max_packet_size)?;
        let buffer = self.take(len)?;
        Some(ReceiveBuffer {
            buffer,
            count_field,
        })
    }

    fn take(&mut self, len: u16) -> Option<Buffer> {
        let end = self.next.checked_add(len)?;
        if end > SIZE {
            return None;
        }
        let buffer = Buffer {
            address: self.next,
            len,
        };
        self.next = end;
        Some(buffer)
    }
}

impl Buffer {
    pub(super) fn control_receive(max_packet_size: u16) -> ReceiveBuffer {
        let limited = max_packet_size.clamp(SMALL_BLOCK, CONTROL_PACKET_LIMIT);
        let (len, count_field) = receive_capacity(limited).unwrap_or((
            CONTROL_PACKET_LIMIT,
            large_count_field(CONTROL_PACKET_LIMIT),
        ));
        ReceiveBuffer {
            buffer: Self {
                address: CONTROL_RECEIVE,
                len,
            },
            count_field,
        }
    }

    pub(super) const fn control_transmit() -> Self {
        Self {
            address: CONTROL_TRANSMIT,
            len: CONTROL_PACKET_LIMIT,
        }
    }

    pub(super) const fn address(self) -> u16 {
        self.address
    }

    pub(super) fn write(self, source: &[u8], mut store: impl FnMut(usize, u16)) -> Option<u16> {
        let len = u16::try_from(source.len())
            .ok()
            .filter(|&len| len <= self.len)?;
        for (offset, pair) in source.chunks(2).enumerate() {
            let halfword = match pair {
                [low, high] => u16::from_le_bytes([*low, *high]),
                [low] => u16::from(*low),
                _ => 0,
            };
            store(self.first_halfword() + offset, halfword);
        }
        Some(len)
    }

    pub(super) fn read(
        self,
        count: u16,
        target: &mut [u8],
        mut load: impl FnMut(usize) -> u16,
    ) -> Option<usize> {
        let count = count & COUNT_MASK;
        if count > self.len {
            return None;
        }
        let len = usize::from(count);
        let received = target.get_mut(..len)?;
        for (offset, pair) in received.chunks_mut(2).enumerate() {
            let bytes = load(self.first_halfword() + offset).to_le_bytes();
            for (byte, value) in pair.iter_mut().zip(bytes) {
                *byte = value;
            }
        }
        Some(len)
    }

    const fn first_halfword(self) -> usize {
        self.address as usize / 2
    }
}

pub(super) const fn transmit_address_slot(index: usize) -> usize {
    index * HALFWORDS_PER_ENTRY
}

pub(super) const fn transmit_count_slot(index: usize) -> usize {
    index * HALFWORDS_PER_ENTRY + 1
}

pub(super) const fn receive_address_slot(index: usize) -> usize {
    index * HALFWORDS_PER_ENTRY + 2
}

pub(super) const fn receive_count_slot(index: usize) -> usize {
    index * HALFWORDS_PER_ENTRY + 3
}

fn receive_capacity(max_packet_size: u16) -> Option<(u16, u16)> {
    match max_packet_size {
        0..=SMALL_BLOCK_LIMIT => {
            let len = max_packet_size
                .max(SMALL_BLOCK)
                .next_multiple_of(SMALL_BLOCK);
            Some((len, (len / SMALL_BLOCK) << BLOCK_COUNT_SHIFT))
        }
        LARGE_BLOCK_FIRST..=LARGE_BLOCK_LIMIT => {
            let len = max_packet_size.next_multiple_of(LARGE_BLOCK);
            Some((len, large_count_field(len)))
        }
        _ => None,
    }
}

const fn large_count_field(len: u16) -> u16 {
    LARGE_BLOCKS | (len / LARGE_BLOCK - 1) << BLOCK_COUNT_SHIFT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_descriptor_table_and_the_control_buffers_come_first() {
        assert_eq!(TABLE_LEN, 64);
        assert_eq!(Buffer::control_receive(64).buffer.address(), 64);
        assert_eq!(Buffer::control_transmit().address(), 128);
        let mut allocator = Allocator::new();
        assert_eq!(allocator.transmit(8).map(Buffer::address), Some(192));
        assert_eq!(allocator.transmit(8).map(Buffer::address), Some(200));
    }

    #[test]
    fn the_stock_endpoint_set_fits_with_room_to_spare() {
        let mut allocator = Allocator::new();
        for max_packet_size in [32, 32, 64, 8, 64] {
            assert!(allocator.transmit(max_packet_size).is_some());
        }
        for max_packet_size in [32, 64] {
            assert!(allocator.receive(max_packet_size).is_some());
        }
        assert_eq!(allocator.next, 488);
        assert!(allocator.transmit(32).is_none());
        assert!(allocator.transmit(24).is_some());
        assert!(allocator.receive(2).is_none());
    }

    #[test]
    fn a_failed_allocation_leaves_the_memory_free() {
        let mut allocator = Allocator::new();
        assert!(allocator.receive(1023).is_none());
        assert!(allocator.transmit(u16::MAX).is_none());
        assert_eq!(allocator.next, FIRST_FREE);
        assert!(allocator.transmit(320).is_some());
        assert!(allocator.transmit(1).is_none());
    }

    #[test]
    fn receive_counts_use_two_byte_blocks_up_to_62_bytes_and_32_byte_blocks_beyond() {
        assert_eq!(receive_capacity(0), Some((2, 0x0400)));
        assert_eq!(receive_capacity(7), Some((8, 0x1000)));
        assert_eq!(receive_capacity(62), Some((62, 0x7C00)));
        assert_eq!(receive_capacity(63), Some((64, 0x8400)));
        assert_eq!(receive_capacity(64), Some((64, 0x8400)));
        assert_eq!(receive_capacity(1024), Some((1024, 0xFC00)));
        assert_eq!(receive_capacity(1025), None);
        assert_eq!(Buffer::control_receive(8).count_field, 0x1000);
        assert_eq!(Buffer::control_receive(512).count_field, 0x8400);
    }

    #[test]
    fn table_slots_follow_the_reference_manual_layout() {
        assert_eq!(transmit_address_slot(0), 0);
        assert_eq!(transmit_count_slot(1), 5);
        assert_eq!(receive_address_slot(2), 10);
        assert_eq!(receive_count_slot(7), 31);
    }

    #[test]
    fn packets_travel_as_little_endian_halfwords() {
        let mut memory = [0_u16; 256];
        let buffer = Buffer {
            address: 200,
            len: 8,
        };
        let written = buffer.write(&[1, 2, 3, 4, 5], |slot, value| {
            if let Some(cell) = memory.get_mut(slot) {
                *cell = value;
            }
        });
        assert_eq!(written, Some(5));
        assert_eq!(memory.get(100..103), Some(&[0x0201, 0x0403, 0x0005][..]));
        let mut target = [0_u8; 8];
        let read = buffer.read(0xFC05, &mut target, |slot| {
            memory.get(slot).copied().unwrap_or(0)
        });
        assert_eq!(read, Some(5));
        assert_eq!(target, [1, 2, 3, 4, 5, 0, 0, 0]);
    }

    #[test]
    fn oversized_packets_are_refused() {
        let buffer = Buffer {
            address: 200,
            len: 4,
        };
        assert_eq!(buffer.write(&[0; 5], |_, _| {}), None);
        assert_eq!(buffer.read(5, &mut [0; 4], |_| 0), None);
    }

    #[test]
    fn a_count_beyond_the_buffer_reads_nothing_of_the_neighbouring_endpoint() {
        let buffer = Buffer {
            address: 200,
            len: 4,
        };
        let mut loads = 0;
        let read = buffer.read(6, &mut [0; 8], |_| {
            loads += 1;
            0xAAAA
        });
        assert_eq!(read, None);
        assert_eq!(loads, 0);
        assert_eq!(buffer.read(4, &mut [0; 8], |_| 0xAAAA), Some(4));
    }
}
