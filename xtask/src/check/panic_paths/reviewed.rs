use std::fmt;

use super::{Origin, location::Location};
use crate::entry_points::Executor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::check) struct Reviewed {
    pub(super) file: &'static str,
    pub(super) line: u32,
    pub(super) function: &'static str,
    pub(super) reached_from: &'static [Origin],
    pub(super) reason: &'static str,
}

const INTERRUPT: &[Origin] = &[Origin::Interrupt];
const DEVICE: &[Origin] = &[Origin::Task(Executor::Device)];
const EXECUTOR_TASKS: &[Origin] = &[
    Origin::Task(Executor::Keys),
    Origin::Task(Executor::Device),
    Origin::Task(Executor::Lighting),
];

const WRITE_FITS: &str = "`write` indexes only after checking that the descriptor fits the buffer";
const CONTROL_PACKET: &str =
    "the control pipe's packet size is 64 bytes, the `max_packet_size_0` of the default `Config`";
const CONTROL_BUFFER: &str = "the control buffer holds 128 bytes (`CONTROL_BUFFER_LEN`), more than \
                              the 4 bytes of any standard answer";
const MS_OS_UNUSED: &str =
    "the device writes no MS OS descriptor through the builder, so the writer stays empty";
const ENDPOINT_BELOW_COUNT: &str =
    "the driver and the HAL index endpoints below the endpoint count, which is below 16";

pub(in crate::check) const REVIEWED: [Reviewed; 75] = [
    Reviewed {
        file: "atomic-waker-1.1.2/src/lib.rs",
        line: 332,
        function: "AtomicWaker::register",
        reached_from: &[Origin::Task(Executor::Keys)],
        reason: "a concurrent `wake` leaves the state `REGISTERING | WAKING` with the waker stored, \
                 which the register path just wrote",
    },
    Reviewed {
        file: "embassy-executor-0.10.0/src/raw/waker.rs",
        line: 35,
        function: "task_from_waker",
        reached_from: EXECUTOR_TASKS,
        reason: "every task that waits on a timer is polled by an embassy executor, whose wakers \
                 these are",
    },
    Reviewed {
        file: "embassy-futures-0.1.2/src/join.rs",
        line: 38,
        function: "MaybeDone::take_output",
        reached_from: DEVICE,
        reason: "`Join::poll` takes the outputs only when every future is done, and each join of \
                 the USB reports holds a future that never completes",
    },
    Reviewed {
        file: "embassy-sync-0.8.0/src/channel.rs",
        line: 845,
        function: "Channel::lock",
        reached_from: EXECUTOR_TASKS,
        reason: "the channel borrows its state only inside its own critical section and never \
                 re-enters it",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/builder.rs",
        line: 276,
        function: "Builder::handler",
        reached_from: DEVICE,
        reason: "the device adds one handler, and the list holds four",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/builder.rs",
        line: 333,
        function: "FunctionBuilder::interface",
        reached_from: DEVICE,
        reason: "`composite_with_iads` is off, so a function has no interface count to update",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/builder.rs",
        line: 342,
        function: "FunctionBuilder::interface",
        reached_from: DEVICE,
        reason: "the device has five interfaces, as many as the `max-interface-count-5` feature \
                 holds",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/builder.rs",
        line: 482,
        function: "InterfaceAltBuilder::alloc_endpoint_in",
        reached_from: DEVICE,
        reason: "the five IN endpoints (addresses 1 to 5, at most 64 bytes) fit the endpoint \
                 slots and memory of both USB controllers",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/builder.rs",
        line: 517,
        function: "InterfaceAltBuilder::alloc_endpoint_out",
        reached_from: DEVICE,
        reason: "the OUT endpoints (addresses 1 and 3, at most 64 bytes) fit the endpoint slots \
                 and memory of both USB controllers",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 95,
        function: "DescriptorWriter::into_buf",
        reached_from: DEVICE,
        reason: "`write` advances the position only over bytes it has checked against the buffer",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 109,
        function: "DescriptorWriter::write",
        reached_from: DEVICE,
        reason: "the configuration descriptor of every face takes at most 156 of the 256 bytes of \
                 `CONFIGURATION_DESCRIPTOR_LEN`, in descriptors of at most 17 bytes",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 114,
        function: "DescriptorWriter::write",
        reached_from: DEVICE,
        reason: WRITE_FITS,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 115,
        function: "DescriptorWriter::write",
        reached_from: DEVICE,
        reason: WRITE_FITS,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 119,
        function: "DescriptorWriter::write",
        reached_from: DEVICE,
        reason: WRITE_FITS,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 151,
        function: "DescriptorWriter::end_configuration",
        reached_from: DEVICE,
        reason: "the buffer starts with the 9-byte configuration header, which the builder writes \
                 first",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 216,
        function: "DescriptorWriter::interface_alt",
        reached_from: DEVICE,
        reason: "the interface count is a byte of the configuration header, which the builder \
                 writes before any interface",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 218,
        function: "DescriptorWriter::interface_alt",
        reached_from: DEVICE,
        reason: "the builder writes the configuration header before any interface",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 259,
        function: "DescriptorWriter::endpoint",
        reached_from: DEVICE,
        reason: "the endpoint count is a byte of the interface descriptor, which `alt_setting` \
                 writes before any of its endpoints",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 260,
        function: "DescriptorWriter::endpoint",
        reached_from: DEVICE,
        reason: "an endpoint is added only through its alternate setting, which writes the \
                 interface descriptor first",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 405,
        function: "BosWriter::capability",
        reached_from: DEVICE,
        reason: "the capability count is a byte of the BOS header, which `bos` wrote at the start \
                 of the 16-byte buffer",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 406,
        function: "BosWriter::capability",
        reached_from: DEVICE,
        reason: "`Builder::new` starts the BOS descriptor before it adds the one capability",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 412,
        function: "BosWriter::capability",
        reached_from: DEVICE,
        reason: "the BOS descriptor, a 5-byte header and one 7-byte capability, takes 12 of the 16 \
                 bytes of `BOS_DESCRIPTOR_LEN`",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 417,
        function: "BosWriter::capability",
        reached_from: DEVICE,
        reason: "the room check above covers the length byte of the capability",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 418,
        function: "BosWriter::capability",
        reached_from: DEVICE,
        reason: "the room check above covers the descriptor type byte of the capability",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 419,
        function: "BosWriter::capability",
        reached_from: DEVICE,
        reason: "the room check above covers the capability type byte",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 422,
        function: "BosWriter::capability",
        reached_from: DEVICE,
        reason: "the room check above covers the data of the capability",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/descriptor.rs",
        line: 432,
        function: "BosWriter::end_bos",
        reached_from: DEVICE,
        reason: "the BOS descriptor, once started, holds at least its 5-byte header",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 374,
        function: "UsbDevice::handle_control_in",
        reached_from: DEVICE,
        reason: CONTROL_PACKET,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 377,
        function: "UsbDevice::handle_control_in",
        reached_from: DEVICE,
        reason: CONTROL_PACKET,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 409,
        function: "UsbDevice::handle_control_out",
        reached_from: DEVICE,
        reason: "the request length is checked against the control buffer first, and the control \
                 pipe's packet size is 64 bytes",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 424,
        function: "UsbDevice::handle_control_out",
        reached_from: DEVICE,
        reason: "each size `data_out` returns fits its chunk of the first `req_length` bytes, \
                 which the control buffer holds",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 536,
        function: "Inner::handle_control_out",
        reached_from: DEVICE,
        reason: "the interface numbers in the configuration descriptor are those of the five \
                 interfaces the builder allocated",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 540,
        function: "Inner::handle_control_out",
        reached_from: DEVICE,
        reason: "SET_CONFIGURATION reads the endpoints of the configuration descriptor, which the \
                 builder wrote well formed",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 558,
        function: "Inner::handle_control_out",
        reached_from: DEVICE,
        reason: "clearing the configuration reads the endpoints of the configuration descriptor, \
                 which the builder wrote well formed",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 593,
        function: "Inner::handle_control_out",
        reached_from: DEVICE,
        reason: "SET_INTERFACE reads the endpoints of the configuration descriptor, which the \
                 builder wrote well formed",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 633,
        function: "Inner::handle_control_in",
        reached_from: DEVICE,
        reason: CONTROL_BUFFER,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 642,
        function: "Inner::handle_control_in",
        reached_from: DEVICE,
        reason: CONTROL_BUFFER,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 655,
        function: "Inner::handle_control_in",
        reached_from: DEVICE,
        reason: CONTROL_BUFFER,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 659,
        function: "Inner::handle_control_in",
        reached_from: DEVICE,
        reason: CONTROL_BUFFER,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 672,
        function: "Inner::handle_control_in",
        reached_from: DEVICE,
        reason: CONTROL_BUFFER,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 728,
        function: "Inner::handle_get_descriptor",
        reached_from: DEVICE,
        reason: CONTROL_BUFFER,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 729,
        function: "Inner::handle_get_descriptor",
        reached_from: DEVICE,
        reason: CONTROL_BUFFER,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 730,
        function: "Inner::handle_get_descriptor",
        reached_from: DEVICE,
        reason: CONTROL_BUFFER,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 731,
        function: "Inner::handle_get_descriptor",
        reached_from: DEVICE,
        reason: CONTROL_BUFFER,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 753,
        function: "Inner::handle_get_descriptor",
        reached_from: DEVICE,
        reason: CONTROL_BUFFER,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/lib.rs",
        line: 758,
        function: "Inner::handle_get_descriptor",
        reached_from: DEVICE,
        reason: "`fitting` cuts every string to the UTF-16 units the control buffer holds, and the \
                 MS OS string has 8",
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/msos.rs",
        line: 72,
        function: "MsOsDescriptorWriter::build",
        reached_from: DEVICE,
        reason: MS_OS_UNUSED,
    },
    Reviewed {
        file: "embassy-usb-0.6.0/src/msos.rs",
        line: 164,
        function: "MsOsDescriptorWriter::end_subset",
        reached_from: DEVICE,
        reason: MS_OS_UNUSED,
    },
    Reviewed {
        file: "embassy-usb-driver-0.2.2/src/lib.rs",
        line: 402,
        function: "EndpointIn::write_transfer",
        reached_from: DEVICE,
        reason: "every IN endpoint is allocated with a maximum packet size of 8 to 64 bytes",
    },
    Reviewed {
        file: "embassy-usb-driver-0.2.2/src/lib.rs",
        line: 405,
        function: "EndpointIn::write_transfer",
        reached_from: DEVICE,
        reason: "every IN endpoint is allocated with a maximum packet size of 8 to 64 bytes",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 54,
        function: "on_interrupt",
        reached_from: INTERRUPT,
        reason: "the controller reports packets only for the endpoints the driver enabled, all below \
                 its endpoint count",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 59,
        function: "on_interrupt",
        reached_from: INTERRUPT,
        reason: "the controller reports a SETUP packet only after receiving its 8 bytes",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 60,
        function: "on_interrupt",
        reached_from: INTERRUPT,
        reason: "SETUP packets arrive only on the control endpoint 0",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 140,
        function: "on_interrupt",
        reached_from: INTERRUPT,
        reason: "`DAINT` raises IN interrupt bits only for the endpoints the driver enabled",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 165,
        function: "on_interrupt",
        reached_from: INTERRUPT,
        reason: "`DAINT` raises OUT interrupt bits only for the endpoints the driver enabled",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 369,
        function: "State::alloc_slot_write",
        reached_from: DEVICE,
        reason: "`alloc_endpoint` checks the index against the endpoint count before it writes \
                 the slot",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 621,
        function: "Driver::start",
        reached_from: DEVICE,
        reason: "slot 0 stays free for the control OUT endpoint, and the FIFOs and the OUT buffer \
                 hold it after ours",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 624,
        function: "Driver::start",
        reached_from: DEVICE,
        reason: "slot 0 stays free for the control IN endpoint, and the FIFOs hold it after ours",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 625,
        function: "Driver::start",
        reached_from: DEVICE,
        reason: "the control OUT endpoint takes slot 0, which no other endpoint can take",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 626,
        function: "Driver::start",
        reached_from: DEVICE,
        reason: "the control IN endpoint takes slot 0, which no other endpoint can take",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 756,
        function: "Bus::config_v1",
        reached_from: DEVICE,
        reason: "the HAL sets up an external ULPI PHY, never the internal high-speed one",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 893,
        function: "Bus::init_fifo",
        reached_from: DEVICE,
        reason: "allocation refuses endpoints beyond the FIFO depth, and `init_fifo` sums the \
                 same sizes",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 1097,
        function: "Bus::endpoint_set_stalled",
        reached_from: DEVICE,
        reason: "the HAL's `Bus` passes on only endpoints below the endpoint count",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 1126,
        function: "Bus::endpoint_is_stalled",
        reached_from: DEVICE,
        reason: "the HAL's `Bus` passes on only endpoints below the endpoint count",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 1142,
        function: "Bus::endpoint_set_enabled",
        reached_from: DEVICE,
        reason: "the HAL's `Bus` passes on only endpoints below the endpoint count",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 1558,
        function: "ControlPipe::data_out",
        reached_from: DEVICE,
        reason: "`read` returns at most the length of the buffer it was given",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/lib.rs",
        line: 1626,
        function: "ep0_mpsiz",
        reached_from: DEVICE,
        reason: "the control packet size is 64 bytes, one of the sizes `ep0_mpsiz` takes",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 255,
        function: "Otg::dieptxf",
        reached_from: DEVICE,
        reason: "`init_fifo` passes the endpoint number minus one, for endpoints below the \
                 endpoint count, which is below 8",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 393,
        function: "Otg::diepctl",
        reached_from: &[Origin::Interrupt, Origin::Task(Executor::Device)],
        reason: "the end-of-frame loop and the driver index endpoints below the endpoint count, \
                 which is below 16",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 399,
        function: "Otg::diepint",
        reached_from: INTERRUPT,
        reason: "endpoint numbers come from the 16 bits of `DAINT`",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 405,
        function: "Otg::dieptsiz",
        reached_from: DEVICE,
        reason: ENDPOINT_BELOW_COUNT,
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 411,
        function: "Otg::dtxfsts",
        reached_from: DEVICE,
        reason: ENDPOINT_BELOW_COUNT,
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 417,
        function: "Otg::doepctl",
        reached_from: DEVICE,
        reason: ENDPOINT_BELOW_COUNT,
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 423,
        function: "Otg::doepint",
        reached_from: INTERRUPT,
        reason: "endpoint numbers come from the 16 bits of `DAINT`",
    },
    Reviewed {
        file: "embassy-usb-synopsys-otg-0.4.0/src/otg_v1.rs",
        line: 429,
        function: "Otg::doeptsiz",
        reached_from: DEVICE,
        reason: ENDPOINT_BELOW_COUNT,
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
        let path = format!("{registry}{}", site.file);
        assert!(site.matches(&location(&path, site.line)));
        assert!(!site.matches(&location(&path, site.line + 1)));
        assert!(!site.matches(&location(&format!("{registry}x{}", site.file), site.line)));
        assert!(!site.matches(&location(
            &format!("/home/owner/openwoot/vendor/{}", site.file),
            site.line
        )));
    }

    #[test]
    fn a_site_is_listed_once_and_names_each_origin_once() {
        for (index, site) in REVIEWED.iter().enumerate() {
            assert!(!site.reached_from.is_empty(), "{site}");
            for (position, origin) in site.reached_from.iter().enumerate() {
                assert!(
                    !site.reached_from[..position].contains(origin),
                    "{site} from {origin:?}"
                );
            }
            assert!(
                REVIEWED[..index]
                    .iter()
                    .all(|earlier| (earlier.file, earlier.line) != (site.file, site.line)),
                "{site}"
            );
        }
    }

    #[test]
    fn only_usb_driver_sites_are_waived_from_interrupts() {
        for site in REVIEWED {
            if site.reached_from.contains(&Origin::Interrupt) {
                assert!(site.file.starts_with("embassy-usb-synopsys-otg-"), "{site}");
            }
        }
    }
}
