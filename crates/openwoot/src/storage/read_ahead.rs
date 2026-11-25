use core::mem;

const WINDOW: u32 = 256;

#[derive(Debug)]
pub(super) struct ReadAhead {
    bytes: [u8; WINDOW as usize],
    held: Span,
    last: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Span {
    pub(super) offset: u32,
    pub(super) len: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Plan {
    Served,
    Refill(Span),
    Direct,
}

impl ReadAhead {
    pub(super) const fn new() -> Self {
        Self {
            bytes: [0; WINDOW as usize],
            held: Span::EMPTY,
            last: Span::EMPTY,
        }
    }

    pub(super) fn plan(
        &mut self,
        offset: u32,
        bytes: &mut [u8],
        end: u32,
        read_size: usize,
    ) -> Plan {
        let Some(request) = Span::of(offset, bytes) else {
            return Plan::Direct;
        };
        let repeated = mem::replace(&mut self.last, request) == request;
        if !repeated && self.copy(request, bytes) {
            return Plan::Served;
        }
        Span::window_for(request, end, read_size).map_or(Plan::Direct, Plan::Refill)
    }

    pub(super) fn window(&mut self, span: Span) -> &mut [u8] {
        self.held = Span::EMPTY;
        self.bytes.get_mut(..span.len as usize).unwrap_or_default()
    }

    pub(super) fn complete(&mut self, span: Span, offset: u32, bytes: &mut [u8]) -> bool {
        self.held = span;
        Span::of(offset, bytes).is_some_and(|request| self.copy(request, bytes))
    }

    pub(super) const fn forget(&mut self) {
        self.held = Span::EMPTY;
        self.last = Span::EMPTY;
    }

    fn copy(&self, request: Span, bytes: &mut [u8]) -> bool {
        if !self.held.holds(request) {
            return false;
        }
        let start = (request.offset - self.held.offset) as usize;
        match self
            .bytes
            .get(start..)
            .and_then(|rest| rest.get(..bytes.len()))
        {
            Some(source) => {
                bytes.copy_from_slice(source);
                true
            }
            None => false,
        }
    }
}

impl Span {
    const EMPTY: Self = Self { offset: 0, len: 0 };

    fn of(offset: u32, bytes: &[u8]) -> Option<Self> {
        let len = u32::try_from(bytes.len())
            .ok()
            .filter(|len| (1..=WINDOW).contains(len))?;
        Some(Self { offset, len })
    }

    fn window_for(request: Self, end: u32, read_size: usize) -> Option<Self> {
        let read_size = u32::try_from(read_size).ok().filter(|size| *size > 0)?;
        let offset = request.offset - request.offset % read_size;
        let room = end.checked_sub(offset)?.min(WINDOW);
        let window = Self {
            offset,
            len: room - room % read_size,
        };
        window.holds(request).then_some(window)
    }

    const fn end(self) -> u32 {
        self.offset.saturating_add(self.len)
    }

    const fn holds(self, inner: Self) -> bool {
        self.offset <= inner.offset && inner.end() <= self.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARTITION: u32 = 4 * 4096;

    fn filled(read_ahead: &mut ReadAhead, offset: u32, len: usize) -> Plan {
        let mut buffer = [0; WINDOW as usize];
        let bytes = &mut buffer[..len];
        let plan = read_ahead.plan(offset, bytes, PARTITION, 1);
        if let Plan::Refill(span) = plan {
            for (index, byte) in read_ahead.window(span).iter_mut().enumerate() {
                *byte = u8::try_from((span.offset as usize + index) % 251).unwrap();
            }
            assert!(read_ahead.complete(span, offset, bytes));
        }
        plan
    }

    #[test]
    fn a_read_starts_a_window_that_serves_the_reads_behind_it() {
        let mut read_ahead = ReadAhead::new();
        assert_eq!(
            filled(&mut read_ahead, 100, 8),
            Plan::Refill(Span {
                offset: 100,
                len: WINDOW
            })
        );
        let mut bytes = [0; 8];
        assert_eq!(read_ahead.plan(108, &mut bytes, PARTITION, 1), Plan::Served);
        assert_eq!(bytes, [108, 109, 110, 111, 112, 113, 114, 115]);
        let last = 100 + WINDOW - 4;
        assert_eq!(
            read_ahead.plan(last, &mut [0; 4], PARTITION, 1),
            Plan::Served
        );
        assert!(matches!(
            read_ahead.plan(last, &mut [0; 8], PARTITION, 1),
            Plan::Refill(_)
        ));
    }

    #[test]
    fn a_read_repeated_at_once_is_read_from_the_flash_again() {
        let mut read_ahead = ReadAhead::new();
        filled(&mut read_ahead, 0, 8);
        assert_eq!(read_ahead.plan(8, &mut [0; 8], PARTITION, 1), Plan::Served);
        assert!(matches!(
            read_ahead.plan(8, &mut [0; 8], PARTITION, 1),
            Plan::Refill(Span { offset: 8, .. })
        ));
    }

    #[test]
    fn the_window_ends_at_the_partition_and_keeps_whole_read_units() {
        let mut read_ahead = ReadAhead::new();
        assert_eq!(
            read_ahead.plan(PARTITION - 8, &mut [0; 8], PARTITION, 1),
            Plan::Refill(Span {
                offset: PARTITION - 8,
                len: 8
            })
        );
        assert_eq!(
            read_ahead.plan(100, &mut [0; 8], 200, 16),
            Plan::Refill(Span {
                offset: 96,
                len: 96
            })
        );
        assert_eq!(
            read_ahead.plan(0, &mut [0; 8], PARTITION, 2 * WINDOW as usize),
            Plan::Direct
        );
    }

    #[test]
    fn empty_reads_and_reads_longer_than_the_window_go_straight_to_the_flash() {
        let mut read_ahead = ReadAhead::new();
        filled(&mut read_ahead, 0, 8);
        assert_eq!(read_ahead.plan(0, &mut [], PARTITION, 1), Plan::Direct);
        assert_eq!(
            read_ahead.plan(0, &mut [0; WINDOW as usize + 1], PARTITION, 1),
            Plan::Direct
        );
        assert_eq!(read_ahead.plan(16, &mut [0; 8], PARTITION, 1), Plan::Served);
    }

    #[test]
    fn a_forgotten_window_serves_nothing() {
        let mut read_ahead = ReadAhead::new();
        filled(&mut read_ahead, 0, 8);
        read_ahead.forget();
        assert!(matches!(
            read_ahead.plan(16, &mut [0; 8], PARTITION, 1),
            Plan::Refill(_)
        ));
    }

    #[test]
    fn a_refill_that_never_completes_serves_nothing() {
        let mut read_ahead = ReadAhead::new();
        filled(&mut read_ahead, 0, 8);
        let Plan::Refill(span) = read_ahead.plan(1024, &mut [0; 8], PARTITION, 1) else {
            panic!("a read outside the window refills it");
        };
        read_ahead.window(span);
        assert!(matches!(
            read_ahead.plan(16, &mut [0; 8], PARTITION, 1),
            Plan::Refill(_)
        ));
    }
}
