use openwoot_protocol::profile::Slot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profiles {
    count: u8,
    active: u8,
    remembered: Option<u8>,
    linked: Option<u8>,
    returning: u8,
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
            linked: None,
            returning: 0,
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
        if active != 0 && !Slot::new(active).is_linked() {
            self.remembered = Some(active);
        } else if let Some(remembered) = self.remembered
            && remembered >= count
        {
            self.remembered = Some(if count < 2 { 1 } else { count - 1 });
        }
    }

    pub const fn link(&mut self, selected: Option<u8>, returning: u8) {
        self.linked = selected;
        self.returning = returning;
    }

    pub const fn select(&mut self, profile: u8) -> bool {
        if profile >= self.count {
            return false;
        }
        self.request(profile)
    }

    pub const fn next(&mut self) -> bool {
        let next = if Slot::new(self.active).is_linked() {
            0
        } else {
            match self.active.checked_add(1) {
                Some(next) if next < self.count => next,
                _ => match self.linked {
                    Some(linked) => linked,
                    None => 0,
                },
            }
        };
        self.request(next)
    }

    pub const fn toggle_mode(&mut self) -> bool {
        let target = if Slot::new(self.active).is_linked() {
            self.returning
        } else if let Some(linked) = self.linked {
            linked
        } else if self.active == 0 {
            match self.remembered {
                Some(profile) => profile,
                None => 1,
            }
        } else {
            self.remembered = Some(self.active);
            0
        };
        self.request(target)
    }

    pub const fn take_request(&mut self) -> Option<u8> {
        self.requested.take()
    }

    const fn request(&mut self, profile: u8) -> bool {
        if profile == self.active || (!Slot::new(profile).is_linked() && profile >= self.count) {
            return false;
        }
        self.requested = Some(profile);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINKED: u8 = 0x82;

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
        assert!(!profiles.select(LINKED));
        let mut single = Profiles::new(1, 0);
        assert!(!single.next());
        let mut full = Profiles::new(u8::MAX, u8::MAX);
        assert!(full.next());
        assert_eq!(full.take_request(), Some(0));
    }

    #[test]
    fn the_mode_toggle_switches_between_the_selected_linked_profile_and_the_one_it_returns_to() {
        let mut profiles = Profiles::new(4, 2);
        profiles.link(Some(LINKED), 2);
        assert!(profiles.toggle_mode());
        assert_eq!(profiles.take_request(), Some(LINKED));
        profiles.set(4, LINKED);
        assert!(profiles.toggle_mode());
        assert_eq!(profiles.take_request(), Some(2));
        profiles.link(None, 2);
        assert!(profiles.toggle_mode());
        assert_eq!(profiles.take_request(), Some(2));
        profiles.set(4, 2);
        assert!(profiles.toggle_mode());
        assert_eq!(profiles.take_request(), Some(0));
    }

    #[test]
    fn next_cycles_through_the_profiles_and_the_selected_linked_one() {
        let mut profiles = Profiles::new(2, 0);
        profiles.set(2, 1);
        assert!(profiles.next());
        assert_eq!(profiles.take_request(), Some(0));
        profiles.link(Some(LINKED), 1);
        assert!(profiles.next());
        assert_eq!(profiles.take_request(), Some(LINKED));
        profiles.set(2, LINKED);
        assert!(profiles.next());
        assert_eq!(profiles.take_request(), Some(0));
        assert_eq!(profiles.remembered, Some(1));
    }
}
