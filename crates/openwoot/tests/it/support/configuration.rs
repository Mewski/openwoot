#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Configuration {
    pub(crate) total_length: u16,
    pub(crate) interface_count: u8,
    pub(crate) value: u8,
    pub(crate) attributes: u8,
    pub(crate) max_power: u8,
    pub(crate) interfaces: Vec<Interface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Interface {
    pub(crate) number: u8,
    pub(crate) alternate: u8,
    pub(crate) endpoint_count: u8,
    pub(crate) class: u8,
    pub(crate) subclass: u8,
    pub(crate) protocol: u8,
    pub(crate) string: u8,
    pub(crate) class_descriptors: Vec<Vec<u8>>,
    pub(crate) endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Endpoint {
    pub(crate) address: u8,
    pub(crate) attributes: u8,
    pub(crate) max_packet_size: u16,
    pub(crate) interval: u8,
}

impl Interface {
    #[must_use]
    pub(crate) fn hid_report_descriptor_length(&self) -> Option<u16> {
        self.class_descriptors
            .iter()
            .find_map(|descriptor| match descriptor.as_slice() {
                [9, 0x21, _, _, _, 1, 0x22, low, high] => Some(u16::from_le_bytes([*low, *high])),
                _ => None,
            })
    }
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Configuration, String> {
    let mut rest = bytes;
    let mut configuration = None;
    let mut interfaces: Vec<Interface> = Vec::new();
    while let [length, descriptor_type, ..] = rest {
        if *length < 2 {
            return Err(format!(
                "descriptor of type 0x{descriptor_type:02x} has length {length}"
            ));
        }
        let (descriptor, tail) = rest
            .split_at_checked(usize::from(*length))
            .ok_or_else(|| format!("descriptor of type 0x{descriptor_type:02x} is cut off"))?;
        rest = tail;
        match descriptor {
            [
                9,
                2,
                total_low,
                total_high,
                interface_count,
                value,
                _,
                attributes,
                max_power,
            ] => {
                configuration = Some(Configuration {
                    total_length: u16::from_le_bytes([*total_low, *total_high]),
                    interface_count: *interface_count,
                    value: *value,
                    attributes: *attributes,
                    max_power: *max_power,
                    interfaces: Vec::new(),
                });
            }
            [
                9,
                4,
                number,
                alternate,
                endpoint_count,
                class,
                subclass,
                protocol,
                string,
            ] => {
                interfaces.push(Interface {
                    number: *number,
                    alternate: *alternate,
                    endpoint_count: *endpoint_count,
                    class: *class,
                    subclass: *subclass,
                    protocol: *protocol,
                    string: *string,
                    class_descriptors: Vec::new(),
                    endpoints: Vec::new(),
                });
            }
            [7, 5, address, attributes, size_low, size_high, interval] => interfaces
                .last_mut()
                .ok_or_else(|| "endpoint outside an interface".to_owned())?
                .endpoints
                .push(Endpoint {
                    address: *address,
                    attributes: *attributes,
                    max_packet_size: u16::from_le_bytes([*size_low, *size_high]),
                    interval: *interval,
                }),
            _ => interfaces
                .last_mut()
                .ok_or_else(|| "class descriptor outside an interface".to_owned())?
                .class_descriptors
                .push(descriptor.to_vec()),
        }
    }
    let mut configuration =
        configuration.ok_or_else(|| "no configuration descriptor".to_owned())?;
    if usize::from(configuration.total_length) != bytes.len() {
        return Err("total length does not match".to_owned());
    }
    configuration.interfaces = interfaces;
    Ok(configuration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_descriptor_shorter_than_its_header_ends_the_walk_with_an_error() {
        let bytes = [
            9, 2, 20, 0, 1, 1, 0, 0xA0, 200, 9, 4, 0, 0, 0, 3, 0, 0, 0, 0, 0x21,
        ];
        assert_eq!(
            parse(&bytes),
            Err("descriptor of type 0x21 has length 0".to_owned())
        );
        let truncated = [9, 2, 11, 0, 1, 1, 0, 0xA0, 200, 1, 4];
        assert_eq!(
            parse(&truncated),
            Err("descriptor of type 0x04 has length 1".to_owned())
        );
    }
}
