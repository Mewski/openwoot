use embassy_usb_driver::{Direction, EndpointType};

const RECEIVED: u16 = 1 << 15;
const RECEIVE_TOGGLE: u16 = 1 << 14;
const RECEIVE_STATUS_SHIFT: u16 = 12;
const SETUP: u16 = 1 << 11;
const KIND_SHIFT: u16 = 9;
const KIND_MASK: u16 = 0b11 << KIND_SHIFT;
const STATUS_OUT: u16 = 1 << 8;
const TRANSMITTED: u16 = 1 << 7;
const TRANSMIT_TOGGLE: u16 = 1 << 6;
const TRANSMIT_STATUS_SHIFT: u16 = 4;
const STATUS_MASK: u16 = 0b11;
const ADDRESS_MASK: u16 = 0b1111;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Disabled,
    Stall,
    Nak,
    Valid,
}

impl Status {
    const fn bits(self) -> u16 {
        match self {
            Self::Disabled => 0b00,
            Self::Stall => 0b01,
            Self::Nak => 0b10,
            Self::Valid => 0b11,
        }
    }

    const fn from_bits(bits: u16) -> Self {
        match bits & STATUS_MASK {
            0b00 => Self::Disabled,
            0b01 => Self::Stall,
            0b10 => Self::Nak,
            _ => Self::Valid,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Toggle(u16);

impl Toggle {
    pub(crate) const NONE: Self = Self(0);

    pub(crate) const fn between(from: Status, to: Status) -> Self {
        Self(from.bits() ^ to.bits())
    }

    pub(crate) const fn then(self, next: Self) -> Self {
        Self(self.0 ^ next.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Reading(pub(crate) u16);

impl Reading {
    pub(crate) const fn status(self, direction: Direction) -> Status {
        match direction {
            Direction::Out => Status::from_bits(self.0 >> RECEIVE_STATUS_SHIFT),
            Direction::In => Status::from_bits(self.0 >> TRANSMIT_STATUS_SHIFT),
        }
    }

    pub(crate) const fn received(self) -> bool {
        self.0 & RECEIVED != 0
    }

    pub(crate) const fn transmitted(self) -> bool {
        self.0 & TRANSMITTED != 0
    }

    pub(crate) const fn setup(self) -> bool {
        self.0 & SETUP != 0
    }

    pub(crate) const fn data_toggle(self, direction: Direction) -> bool {
        self.0 & data_toggle_mask(direction) != 0
    }

    pub(crate) const fn acknowledge(self) -> u16 {
        self.preserve() & !(self.0 & (RECEIVED | TRANSMITTED))
    }

    pub(crate) const fn change_status(self, direction: Direction, wanted: Status) -> u16 {
        self.preserve() | status_toggle(direction, Toggle::between(self.status(direction), wanted))
    }

    pub(crate) const fn restart(self, direction: Direction, wanted: Status) -> u16 {
        self.change_status(direction, wanted) | (self.0 & data_toggle_mask(direction))
    }

    const fn preserve(self) -> u16 {
        (self.0 & (KIND_MASK | STATUS_OUT | ADDRESS_MASK)) | RECEIVED | TRANSMITTED
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Command {
    kind: u16,
    address: u16,
    status_out: bool,
    receive: Toggle,
    transmit: Toggle,
}

impl Command {
    pub(crate) fn new(endpoint_type: EndpointType, index: usize) -> Self {
        Self {
            kind: kind_bits(endpoint_type),
            address: u16::try_from(index).unwrap_or(0) & ADDRESS_MASK,
            status_out: false,
            receive: Toggle::NONE,
            transmit: Toggle::NONE,
        }
    }

    pub(crate) const fn with_status_out(self, status_out: bool) -> Self {
        Self { status_out, ..self }
    }

    pub(crate) const fn toggle_receive(self, receive: Toggle) -> Self {
        Self { receive, ..self }
    }

    pub(crate) const fn toggle_transmit(self, transmit: Toggle) -> Self {
        Self { transmit, ..self }
    }

    pub(crate) const fn bits(self) -> u16 {
        let status_out = if self.status_out { STATUS_OUT } else { 0 };
        RECEIVED
            | TRANSMITTED
            | self.kind << KIND_SHIFT
            | status_out
            | self.address
            | status_toggle(Direction::Out, self.receive)
            | status_toggle(Direction::In, self.transmit)
    }
}

const fn data_toggle_mask(direction: Direction) -> u16 {
    match direction {
        Direction::Out => RECEIVE_TOGGLE,
        Direction::In => TRANSMIT_TOGGLE,
    }
}

const fn status_toggle(direction: Direction, toggle: Toggle) -> u16 {
    match direction {
        Direction::Out => toggle.0 << RECEIVE_STATUS_SHIFT,
        Direction::In => toggle.0 << TRANSMIT_STATUS_SHIFT,
    }
}

const fn kind_bits(endpoint_type: EndpointType) -> u16 {
    match endpoint_type {
        EndpointType::Bulk => 0b00,
        EndpointType::Control => 0b01,
        EndpointType::Isochronous => 0b10,
        EndpointType::Interrupt => 0b11,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const FLAGS: u16 = RECEIVED | TRANSMITTED;
    const TOGGLED: u16 = RECEIVE_TOGGLE
        | TRANSMIT_TOGGLE
        | STATUS_MASK << RECEIVE_STATUS_SHIFT
        | STATUS_MASK << TRANSMIT_STATUS_SHIFT;
    const STATUSES: [Status; 4] = [Status::Disabled, Status::Stall, Status::Nak, Status::Valid];
    const DIRECTIONS: [Direction; 2] = [Direction::Out, Direction::In];

    pub(crate) fn hardware_write(current: u16, written: u16) -> u16 {
        let kept_flags = current & written & FLAGS;
        let toggled = (current ^ written) & TOGGLED;
        let read_only = current & SETUP;
        let plain = written & !(FLAGS | TOGGLED | SETUP);
        kept_flags | toggled | read_only | plain
    }

    fn register(receive: Status, transmit: Status, extra: u16) -> u16 {
        receive.bits() << RECEIVE_STATUS_SHIFT | transmit.bits() << TRANSMIT_STATUS_SHIFT | extra
    }

    pub(crate) fn after_setup() -> u16 {
        register(Status::Nak, Status::Nak, SETUP | 0b01 << KIND_SHIFT)
    }

    pub(crate) fn status_out(value: u16) -> bool {
        value & STATUS_OUT != 0
    }

    fn other(direction: Direction) -> Direction {
        match direction {
            Direction::Out => Direction::In,
            Direction::In => Direction::Out,
        }
    }

    #[test]
    fn a_status_change_reaches_the_wanted_status_and_keeps_everything_else() {
        let extra = RECEIVED | RECEIVE_TOGGLE | SETUP | 0b11 << KIND_SHIFT | TRANSMIT_TOGGLE | 5;
        for direction in DIRECTIONS {
            for current in STATUSES {
                for wanted in STATUSES {
                    for neighbour in STATUSES {
                        let before = match direction {
                            Direction::Out => register(current, neighbour, extra),
                            Direction::In => register(neighbour, current, extra),
                        };
                        let write = Reading(before).change_status(direction, wanted);
                        let after = Reading(hardware_write(before, write));
                        assert_eq!(after.status(direction), wanted);
                        assert_eq!(after.status(other(direction)), neighbour);
                        assert_eq!(after.0 & !TOGGLED, before & !TOGGLED);
                        assert_eq!(
                            after.0 & (RECEIVE_TOGGLE | TRANSMIT_TOGGLE),
                            before & (RECEIVE_TOGGLE | TRANSMIT_TOGGLE)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_restart_also_resets_the_data_toggle_of_its_direction_only() {
        let before = register(
            Status::Stall,
            Status::Stall,
            RECEIVE_TOGGLE | TRANSMIT_TOGGLE | 3,
        );
        let write = Reading(before).restart(Direction::In, Status::Nak);
        let after = Reading(hardware_write(before, write));
        assert_eq!(after.status(Direction::In), Status::Nak);
        assert_eq!(after.status(Direction::Out), Status::Stall);
        assert_eq!(after.0 & TRANSMIT_TOGGLE, 0);
        assert_eq!(after.0 & RECEIVE_TOGGLE, RECEIVE_TOGGLE);
        let write = Reading(after.0).restart(Direction::Out, Status::Valid);
        let after = Reading(hardware_write(after.0, write));
        assert_eq!(after.status(Direction::Out), Status::Valid);
        assert_eq!(after.0 & (RECEIVE_TOGGLE | TRANSMIT_TOGGLE), 0);
    }

    #[test]
    fn acknowledging_clears_only_the_flags_that_were_read() {
        let seen = register(Status::Nak, Status::Nak, RECEIVED | 2);
        let write = Reading(seen).acknowledge();
        let raised_since = seen | TRANSMITTED;
        let after = Reading(hardware_write(raised_since, write));
        assert!(!after.received());
        assert!(after.transmitted());
        assert_eq!(after.0 & !FLAGS, seen & !FLAGS);
    }

    #[test]
    fn a_status_change_from_a_stale_read_keeps_a_new_completion_flag() {
        let seen = register(Status::Valid, Status::Nak, 1);
        let write = Reading(seen).change_status(Direction::In, Status::Stall);
        let after = Reading(hardware_write(seen | RECEIVED, write));
        assert!(after.received());
        assert_eq!(after.status(Direction::In), Status::Stall);
    }

    #[test]
    fn commands_write_type_address_and_status_out_and_never_clear_flags() {
        let before = register(Status::Nak, Status::Nak, RECEIVED | TRANSMITTED | SETUP);
        let command = Command::new(EndpointType::Control, 0)
            .with_status_out(true)
            .toggle_receive(
                Toggle::between(Status::Nak, Status::Stall)
                    .then(Toggle::between(Status::Stall, Status::Valid)),
            )
            .toggle_transmit(Toggle::between(Status::Nak, Status::Valid));
        let after = Reading(hardware_write(before, command.bits()));
        assert_eq!(after.status(Direction::Out), Status::Valid);
        assert_eq!(after.status(Direction::In), Status::Valid);
        assert!(after.received() && after.transmitted() && after.setup());
        assert_eq!(after.0 & KIND_MASK, 0b01 << KIND_SHIFT);
        assert_eq!(after.0 & STATUS_OUT, STATUS_OUT);
    }

    #[test]
    fn endpoint_types_carry_the_reference_manual_encodings() {
        assert_eq!(Command::new(EndpointType::Bulk, 1).bits() & !FLAGS, 1);
        assert_eq!(
            Command::new(EndpointType::Control, 2).bits() & !FLAGS,
            0x0202
        );
        assert_eq!(
            Command::new(EndpointType::Isochronous, 3).bits() & !FLAGS,
            0x0403
        );
        assert_eq!(
            Command::new(EndpointType::Interrupt, 7).bits() & !FLAGS,
            0x0607
        );
        assert_eq!(
            Command::new(EndpointType::Interrupt, 9).bits() & ADDRESS_MASK,
            9
        );
    }
}
