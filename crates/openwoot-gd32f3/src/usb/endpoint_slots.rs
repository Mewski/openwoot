use embassy_usb_driver::{Direction, EndpointType};

use super::packet_memory::ENDPOINT_COUNT;

#[derive(Clone, Copy, Debug, Default)]
struct Slot {
    endpoint_type: Option<EndpointType>,
    used_in: bool,
    used_out: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Slots([Slot; ENDPOINT_COUNT]);

impl Slots {
    pub(crate) fn find(
        &self,
        endpoint_type: EndpointType,
        direction: Direction,
        requested: Option<usize>,
    ) -> Option<usize> {
        let fits = |index: usize| {
            self.0
                .get(index)
                .is_some_and(|slot| index != 0 && slot.accepts(endpoint_type, direction))
        };
        match requested {
            Some(index) => fits(index).then_some(index),
            None => (0..ENDPOINT_COUNT).find(|&index| fits(index)),
        }
    }

    pub(crate) fn claim(
        &mut self,
        index: usize,
        endpoint_type: EndpointType,
        direction: Direction,
    ) {
        if let Some(slot) = self.0.get_mut(index) {
            slot.endpoint_type = Some(endpoint_type);
            match direction {
                Direction::In => slot.used_in = true,
                Direction::Out => slot.used_out = true,
            }
        }
    }

    pub(crate) fn types(&self) -> [EndpointType; ENDPOINT_COUNT] {
        self.0
            .map(|slot| slot.endpoint_type.unwrap_or(EndpointType::Bulk))
    }
}

impl Slot {
    fn accepts(self, endpoint_type: EndpointType, direction: Direction) -> bool {
        if endpoint_type == EndpointType::Isochronous {
            return false;
        }
        let used_here = match direction {
            Direction::In => self.used_in,
            Direction::Out => self.used_out,
        };
        match self.endpoint_type {
            None => true,
            Some(existing) => existing == endpoint_type && !used_here,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_zero_stays_with_the_control_pipe() {
        let slots = Slots::default();
        assert_eq!(
            slots.find(EndpointType::Control, Direction::Out, Some(0)),
            None
        );
        assert_eq!(
            slots.find(EndpointType::Interrupt, Direction::In, None),
            Some(1)
        );
    }

    #[test]
    fn requested_addresses_are_honoured_and_shared_by_both_directions_of_one_type() {
        let mut slots = Slots::default();
        assert_eq!(
            slots.find(EndpointType::Interrupt, Direction::In, Some(3)),
            Some(3)
        );
        slots.claim(3, EndpointType::Interrupt, Direction::In);
        assert_eq!(
            slots.find(EndpointType::Interrupt, Direction::In, Some(3)),
            None
        );
        assert_eq!(
            slots.find(EndpointType::Bulk, Direction::Out, Some(3)),
            None
        );
        assert_eq!(
            slots.find(EndpointType::Interrupt, Direction::Out, Some(3)),
            Some(3)
        );
        slots.claim(3, EndpointType::Interrupt, Direction::Out);
        assert_eq!(
            slots.find(EndpointType::Interrupt, Direction::Out, Some(3)),
            None
        );
        assert_eq!(
            slots.find(EndpointType::Interrupt, Direction::Out, Some(8)),
            None
        );
    }

    #[test]
    fn free_endpoints_are_taken_in_order_until_none_is_left() {
        let mut slots = Slots::default();
        for expected in 1..ENDPOINT_COUNT {
            let found = slots.find(EndpointType::Bulk, Direction::In, None);
            assert_eq!(found, Some(expected));
            slots.claim(expected, EndpointType::Bulk, Direction::In);
        }
        assert_eq!(slots.find(EndpointType::Bulk, Direction::In, None), None);
        assert_eq!(
            slots.find(EndpointType::Bulk, Direction::Out, None),
            Some(1)
        );
    }

    #[test]
    fn isochronous_endpoints_are_refused() {
        let slots = Slots::default();
        assert_eq!(
            slots.find(EndpointType::Isochronous, Direction::In, None),
            None
        );
    }

    #[test]
    fn unused_slots_are_configured_as_bulk() {
        let mut slots = Slots::default();
        slots.claim(2, EndpointType::Interrupt, Direction::In);
        let types = slots.types();
        assert_eq!(types.get(2), Some(&EndpointType::Interrupt));
        assert_eq!(types.get(1), Some(&EndpointType::Bulk));
    }
}
