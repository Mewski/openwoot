use std::time::{Duration, Instant};

const REDRAW_INTERVAL: Duration = Duration::from_millis(250);
const HIDDEN_AFTER: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy)]
pub(crate) struct Visibility {
    rendered_at: Instant,
    notified_at: Option<Instant>,
}

impl Visibility {
    pub(crate) const fn new(now: Instant) -> Self {
        Self {
            rendered_at: now,
            notified_at: None,
        }
    }

    pub(crate) const fn rendered(&mut self, now: Instant) {
        self.rendered_at = now;
    }

    pub(crate) const fn restart(&mut self, now: Instant) {
        self.rendered_at = now;
        self.notified_at = None;
    }

    pub(crate) fn is_shown(&self, now: Instant) -> bool {
        self.notified_at.is_none_or(|notified| {
            self.rendered_at >= notified || now.duration_since(notified) < HIDDEN_AFTER
        })
    }

    pub(crate) fn redraw_due(&self, now: Instant) -> bool {
        self.notified_at
            .is_none_or(|notified| now.duration_since(notified) >= REDRAW_INTERVAL)
    }

    pub(crate) fn notified(&mut self, now: Instant) {
        if self
            .notified_at
            .is_none_or(|notified| self.rendered_at >= notified)
        {
            self.notified_at = Some(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after(start: Instant, milliseconds: u64) -> Instant {
        start + Duration::from_millis(milliseconds)
    }

    #[test]
    fn a_page_that_renders_after_each_notification_stays_shown() {
        let start = Instant::now();
        let mut visibility = Visibility::new(start);
        assert!(visibility.is_shown(after(start, 5000)));
        visibility.notified(after(start, 250));
        visibility.rendered(after(start, 260));
        assert!(visibility.is_shown(after(start, 900)));
    }

    #[test]
    fn a_page_that_stops_rendering_is_hidden_after_half_a_second() {
        let start = Instant::now();
        let mut visibility = Visibility::new(start);
        visibility.notified(after(start, 250));
        visibility.notified(after(start, 600));
        assert!(visibility.is_shown(after(start, 700)));
        assert!(!visibility.is_shown(after(start, 760)));
        visibility.restart(after(start, 800));
        assert!(visibility.is_shown(after(start, 2000)));
    }

    #[test]
    fn a_redraw_is_due_a_quarter_second_after_the_last_notification() {
        let start = Instant::now();
        let mut visibility = Visibility::new(start);
        assert!(visibility.redraw_due(start));
        visibility.notified(start);
        assert!(!visibility.redraw_due(after(start, 200)));
        assert!(visibility.redraw_due(after(start, 250)));
    }
}
