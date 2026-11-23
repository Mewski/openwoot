use std::fmt;

use super::{Origin, location::Location};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::check) struct Reviewed {
    pub(super) file: &'static str,
    pub(super) line: u32,
    pub(super) function: &'static str,
    pub(super) reached_from: Origin,
    pub(super) reason: &'static str,
}

pub(in crate::check) const REVIEWED: [Reviewed; 11] = [
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 54,
        function: "on_interrupt",
        reached_from: Origin::Interrupt,
        reason: "the controller reports packets only for the endpoints the driver enabled, all below \
                 its endpoint count",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 59,
        function: "on_interrupt",
        reached_from: Origin::Interrupt,
        reason: "the controller reports a SETUP packet only after receiving its 8 bytes",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 60,
        function: "on_interrupt",
        reached_from: Origin::Interrupt,
        reason: "SETUP packets arrive only on the control endpoint 0",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 140,
        function: "on_interrupt",
        reached_from: Origin::Interrupt,
        reason: "`DAINT` raises IN interrupt bits only for the endpoints the driver enabled",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 165,
        function: "on_interrupt",
        reached_from: Origin::Interrupt,
        reason: "`DAINT` raises OUT interrupt bits only for the endpoints the driver enabled",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 393,
        function: "Otg::diepctl",
        reached_from: Origin::Interrupt,
        reason: "the end-of-frame loop indexes endpoints below the endpoint count, which is below 16",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 399,
        function: "Otg::diepint",
        reached_from: Origin::Interrupt,
        reason: "endpoint numbers come from the 16 bits of `DAINT`",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 423,
        function: "Otg::doepint",
        reached_from: Origin::Interrupt,
        reason: "endpoint numbers come from the 16 bits of `DAINT`",
    },
    Reviewed {
        file: "atomic-waker-1.1.2/src/lib.rs",
        line: 332,
        function: "AtomicWaker::register",
        reached_from: Origin::KeyTask,
        reason: "a concurrent `wake` leaves the state `REGISTERING | WAKING` with the waker stored, \
                 which the register path just wrote",
    },
    Reviewed {
        file: "embassy-sync-0.8.0/src/channel.rs",
        line: 845,
        function: "Channel::lock",
        reached_from: Origin::KeyTask,
        reason: "the channel borrows its state only inside its own critical section and never \
                 re-enters it",
    },
    Reviewed {
        file: "embassy-executor-0.10.0/src/raw/waker.rs",
        line: 35,
        function: "task_from_waker",
        reached_from: Origin::KeyTask,
        reason: "every task that waits on a timer is polled by an embassy executor, whose wakers \
                 these are",
    },
];

impl Reviewed {
    pub(super) fn matches(&self, location: &Location) -> bool {
        location.line == self.line && location.dependency_file() == Some(self.file)
    }
}

impl fmt::Display for Reviewed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}:{}` in `{}` ({})",
            self.file, self.line, self.function, self.reason
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_versioned_crate_directory(directory: &str) -> bool {
        directory.rsplit_once('-').is_some_and(|(name, version)| {
            !name.is_empty()
                && version.split('.').count() == 3
                && version.split('.').all(|part| part.parse::<u32>().is_ok())
        })
    }

    #[test]
    fn reviewed_sites_lie_in_pinned_dependencies() {
        for site in REVIEWED {
            let directory = site.file.split('/').next().unwrap_or_default();
            assert!(is_versioned_crate_directory(directory), "{site}");
            assert!(!site.function.is_empty() && !site.reason.is_empty());
        }
    }

    #[test]
    fn a_site_matches_its_dependency_file_and_line_only() {
        let site = REVIEWED[0];
        let location = |file: &str, line| Location {
            file: file.to_owned(),
            line,
            column: 9,
        };
        let registry = "/home/owner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/";
        assert!(site.matches(&location(&format!("{registry}{}", site.file), 54)));
        assert!(!site.matches(&location(&format!("{registry}{}", site.file), 55)));
        assert!(!site.matches(&location(&format!("{registry}x{}", site.file), 54)));
        assert!(!site.matches(&location(
            &format!("/home/owner/openwoot/vendor/{}", site.file),
            54
        )));
    }

    #[test]
    fn usb_sites_are_waived_from_interrupts_and_the_others_from_the_key_task() {
        for (index, site) in REVIEWED.iter().enumerate() {
            let expected = if index < 8 {
                Origin::Interrupt
            } else {
                Origin::KeyTask
            };
            assert_eq!(site.reached_from, expected, "{site}");
        }
    }
}
