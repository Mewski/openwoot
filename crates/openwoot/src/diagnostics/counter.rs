use core::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Default)]
pub struct Counter(AtomicU32);

impl Counter {
    #[must_use]
    pub const fn new() -> Self {
        Self(AtomicU32::new(0))
    }

    #[inline]
    pub fn increment(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn value(&self) -> u32 {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_counter_wraps_instead_of_stopping() {
        let counter = Counter(AtomicU32::new(u32::MAX));
        counter.increment();
        assert_eq!(counter.value(), 0);
        counter.increment();
        assert_eq!(counter.value(), 1);
    }
}
