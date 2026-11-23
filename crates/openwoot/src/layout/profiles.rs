#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profiles {
    count: u8,
    active: u8,
    remembered: Option<u8>,
    requested: Option<u8>,
}

impl Default for Profiles {
    fn default() -> Self {
        Self::new(1, 0)
    }
}

impl Profiles {
    #[must_use]
    pub const fn new(count: u8, active: u8) -> Self {
        Self {
            count,
            active,
            remembered: None,
            requested: None,
        }
    }

    #[must_use]
    pub const fn count(&self) -> u8 {
        self.count
    }

    #[must_use]
    pub const fn active(&self) -> u8 {
        self.active
    }

    pub const fn set(&mut self, count: u8, active: u8) {
        self.count = count;
        self.active = active;
        self.requested = None;
        if active != 0 {
            self.remembered = Some(active);
        } else if let Some(remembered) = self.remembered
            && remembered >= count
        {
            self.remembered = Some(if count < 2 { 1 } else { count - 1 });
        }
    }

    pub const fn select(&mut self, profile: u8) -> bool {
        if profile >= self.count || profile == self.active {
            return false;
        }
        self.requested = Some(profile);
        true
    }

    pub const fn next(&mut self) -> bool {
        let next = match self.active.checked_add(1) {
            Some(next) if next < self.count => next,
            _ => 0,
        };
        self.select(next)
    }

    pub const fn toggle_mode(&mut self) -> bool {
        let target = if self.active == 0 {
            match self.remembered {
                Some(profile) => profile,
                None => 1,
            }
        } else {
            self.remembered = Some(self.active);
            0
        };
        self.select(target)
    }

    pub const fn take_request(&mut self) -> Option<u8> {
        self.requested.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_toggle_returns_to_the_remembered_profile() {
        let mut profiles = Profiles::new(4, 2);
        assert!(profiles.toggle_mode());
        assert_eq!(profiles.take_request(), Some(0));
        profiles.set(4, 0);
        assert!(profiles.toggle_mode());
        assert_eq!(profiles.take_request(), Some(2));
        let mut fresh = Profiles::new(4, 0);
        assert!(fresh.toggle_mode());
        assert_eq!(fresh.take_request(), Some(1));
    }

    #[test]
    fn any_activation_of_another_profile_becomes_the_toggle_target() {
        let mut profiles = Profiles::new(4, 0);
        profiles.set(4, 3);
        profiles.set(4, 0);
        assert!(profiles.toggle_mode());
        assert_eq!(profiles.take_request(), Some(3));
        profiles.set(2, 0);
        assert!(profiles.toggle_mode());
        assert_eq!(profiles.take_request(), Some(1));
    }

    #[test]
    fn next_wraps_and_selection_skips_the_active_and_missing_profiles() {
        let mut profiles = Profiles::new(4, 3);
        assert!(profiles.next());
        assert_eq!(profiles.take_request(), Some(0));
        assert!(!profiles.select(3));
        assert!(!profiles.select(4));
        let mut single = Profiles::new(1, 0);
        assert!(!single.next());
        let mut full = Profiles::new(u8::MAX, u8::MAX);
        assert!(full.next());
        assert_eq!(full.take_request(), Some(0));
    }
}
