#![allow(
    clippy::cast_possible_truncation,
    clippy::inline_always,
    reason = "the sample buffer address fits 32 bits on this family, each word packs two 16-bit \
              conversions, and the sample read is inlined into the scan interrupt as the C HAL's \
              always_inline helpers"
)]

use core::sync::atomic::{AtomicU32, Ordering};

use openwoot::scan::Sample;

use super::adc::{self, MAX_RANKS};
use crate::dma;

type Channel = dma::Channel<0, 0>;

static SAMPLES: [AtomicU32; MAX_RANKS] = [const { AtomicU32::new(0) }; MAX_RANKS];

pub(super) fn start(count: u32) {
    let channel = Channel::new();
    channel.enable_clock();
    channel.clear_control(dma::ENABLE);
    channel.write_control(0);
    channel.set_count(0);
    channel.set_peripheral_address(0);
    channel.set_memory_address(0);
    channel.clear_flags();
    channel.set_peripheral_address(adc::sample_address());
    channel.set_memory_address(SAMPLES.as_ptr().addr() as u32);
    channel.set_count(count);
    channel.modify_control(|bits| {
        (bits & !(dma::PERIPHERAL_WIDTH | dma::MEMORY_WIDTH | dma::PRIORITY))
            | dma::PERIPHERAL_WIDTH_32
            | dma::MEMORY_WIDTH_32
            | dma::PRIORITY_HIGH
    });
    channel.clear_control(dma::PERIPHERAL_INCREMENT);
    channel.set_control(dma::MEMORY_INCREMENT);
    channel.clear_control(dma::MEMORY_TO_PERIPHERAL);
    channel.set_control(dma::CIRCULAR);
    channel.set_control(dma::ENABLE);
}

pub(super) fn stop() {
    Channel::new().clear_control(dma::ENABLE);
}

#[inline(always)]
pub(super) fn sample(rank: usize) -> Sample {
    let word = SAMPLES
        .get(rank)
        .map_or(0, |sample| sample.load(Ordering::Relaxed));
    Sample {
        reading: word as u16,
        reference: (word >> 16) as u16,
    }
}
