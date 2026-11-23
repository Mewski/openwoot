#![allow(
    clippy::cast_possible_truncation,
    clippy::inline_always,
    reason = "addresses fit 32 bits on this family, a sample is the low 16 bits of its word, and \
              the sample read is inlined into the scan interrupt to keep it short"
)]

use core::sync::atomic::{AtomicU32, Ordering};

use super::adc::{self, MAX_RANKS};
use crate::dma;

type Channel = dma::Channel<1, 0>;

const DISABLE_SPINS: u32 = 1_000;

static SAMPLES: [AtomicU32; MAX_RANKS] = [const { AtomicU32::new(0) }; MAX_RANKS];
static COUNT: AtomicU32 = AtomicU32::new(1);

pub(super) fn enable_clock() {
    Channel::new().enable_clock();
}

pub(super) fn start(count: u32) {
    COUNT.store(count, Ordering::Relaxed);
    let channel = Channel::new();
    channel.clear_control(dma::ENABLE);
    let mut spins = 0;
    while channel.control() & dma::ENABLE != 0 && spins < DISABLE_SPINS {
        spins += 1;
    }
    channel.write_control(0);
    channel.set_count(0);
    channel.set_peripheral_address(0);
    channel.set_memory_address(0);
    channel.set_second_memory_address(0);
    channel.write_fifo_control(0);
    channel.clear_flags();
    channel.clear_fifo_control(dma::FIFO_MULTI_DATA);
    channel.set_peripheral_address(adc::sample_address());
    channel.set_memory_address(SAMPLES.as_ptr().addr() as u32);
    channel.set_count(count);
    channel.modify_control(|bits| {
        (bits & !(dma::PERIPHERAL_WIDTH | dma::MEMORY_WIDTH | dma::PRIORITY | dma::DIRECTION))
            | dma::PERIPHERAL_WIDTH_32
            | dma::MEMORY_WIDTH_32
            | dma::PRIORITY_ULTRA_HIGH
    });
    channel.clear_control(dma::PERIPHERAL_INCREMENT);
    channel.set_control(dma::MEMORY_INCREMENT);
    channel.clear_control(dma::CIRCULAR);
    channel.clear_control(dma::REQUEST);
    channel.set_control(dma::CIRCULAR);
    channel.set_control(dma::ENABLE);
}

pub(super) fn restart() {
    start(COUNT.load(Ordering::Relaxed));
}

#[inline(always)]
pub(super) fn reading(rank: usize) -> u16 {
    SAMPLES
        .get(rank)
        .map_or(0, |sample| sample.load(Ordering::Relaxed)) as u16
}
