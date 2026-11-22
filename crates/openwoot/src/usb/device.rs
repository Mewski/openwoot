use embassy_usb::{
    Builder, Config, UsbDevice, UsbVersion,
    driver::{Direction, Driver, EndpointAddress},
};

use super::{
    Face, HostState, Identity, KeyboardFormat, Mode,
    control::Control,
    hid_class,
    interfaces::{
        CHANNEL_ENDPOINT, CHANNEL_INTERFACE, CONTROLS_ENDPOINT, CONTROLS_INTERFACE,
        GAMEPAD_ENDPOINT, GAMEPAD_INTERFACE, KEY_STATE_ENDPOINT, KEY_STATE_INTERFACE,
        KEYBOARD_ENDPOINT, KEYBOARD_INTERFACE,
    },
};
use crate::{hid, xinput};

const CONFIGURATION_DESCRIPTOR_LEN: usize = 256;
const BOS_DESCRIPTOR_LEN: usize = 16;
const CONTROL_BUFFER_LEN: usize = 128;
const MAX_POWER_MILLIAMPERES: u16 = 400;
const GAMEPAD_MAX_PACKET_SIZE: u16 = 32;
const PLACEHOLDER_INTERVAL: u8 = 10;
const NKRO_INTERVAL: u8 = 1;
const BOOT_KEYBOARD_INTERVAL: u8 = 10;
const CHANNEL_MAX_PACKET_SIZE: u16 = 64;
const CHANNEL_INTERVAL: u8 = 1;
const CONTROLS_MAX_PACKET_SIZE: u16 = 8;
const CONTROLS_INTERVAL: u8 = 4;
const KEY_STATE_INTERVAL: u8 = 1;

pub struct Resources<'d> {
    configuration_descriptor: [u8; CONFIGURATION_DESCRIPTOR_LEN],
    bos_descriptor: [u8; BOS_DESCRIPTOR_LEN],
    control_buffer: [u8; CONTROL_BUFFER_LEN],
    control: Option<Control<'d>>,
}

pub struct ChannelEndpoints<'d, D: Driver<'d>> {
    pub output: D::EndpointOut,
    pub input: D::EndpointIn,
}

pub enum Gamepad<'d, D: Driver<'d>> {
    Xinput {
        input: D::EndpointIn,
        output: D::EndpointOut,
    },
    Hid(D::EndpointIn),
    Placeholder(D::EndpointIn),
}

pub struct Device<'d, D: Driver<'d>> {
    pub usb: UsbDevice<'d, D>,
    pub gamepad: Gamepad<'d, D>,
    pub keyboard: D::EndpointIn,
    pub channel: ChannelEndpoints<'d, D>,
    pub controls: D::EndpointIn,
    pub key_states: D::EndpointIn,
}

impl Resources<'_> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            configuration_descriptor: [0; CONFIGURATION_DESCRIPTOR_LEN],
            bos_descriptor: [0; BOS_DESCRIPTOR_LEN],
            control_buffer: [0; CONTROL_BUFFER_LEN],
            control: None,
        }
    }
}

impl Default for Resources<'_> {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build<'d, D: Driver<'d>>(
    driver: D,
    face: Face,
    identity: Identity<'d>,
    resources: &'d mut Resources<'d>,
    host: &'d HostState,
) -> Device<'d, D> {
    let Resources {
        configuration_descriptor,
        bos_descriptor,
        control_buffer,
        control,
    } = resources;
    let control = control.insert(Control::new(host, face.mode == Mode::Xinput));
    let mut builder = Builder::new(
        driver,
        device_configuration(face, identity),
        configuration_descriptor,
        bos_descriptor,
        &mut [],
        control_buffer,
    );

    let gamepad = gamepad_interface(&mut builder, control, face);
    let keyboard = keyboard_interface(&mut builder, control, face.keyboard);
    let channel = channel_interface(&mut builder, control);
    let controls = hid_input_interface(
        &mut builder,
        control,
        CONTROLS_INTERFACE,
        hid::CONTROLS_DESCRIPTOR,
        CONTROLS_ENDPOINT,
        CONTROLS_MAX_PACKET_SIZE,
        CONTROLS_INTERVAL,
    );
    let key_states = hid_input_interface(
        &mut builder,
        control,
        KEY_STATE_INTERFACE,
        hid::KEY_STATE_DESCRIPTOR,
        KEY_STATE_ENDPOINT,
        max_packet_size(hid::KEY_STATE_LEN),
        KEY_STATE_INTERVAL,
    );
    builder.handler(control);

    Device {
        usb: builder.build(),
        gamepad,
        keyboard,
        channel,
        controls,
        key_states,
    }
}

fn device_configuration(face: Face, identity: Identity<'_>) -> Config<'_> {
    let mut configuration = Config::new(
        identity.ids.vendor_id,
        identity.ids.product_id_base + face.mode.product_id_offset(),
    );
    configuration.bcd_usb = UsbVersion::Two;
    configuration.device_class = 0;
    configuration.device_sub_class = 0;
    configuration.device_protocol = 0;
    configuration.composite_with_iads = false;
    configuration.device_release = face.keyboard.device_release();
    configuration.manufacturer = Some(identity.manufacturer);
    configuration.product = Some(identity.product);
    configuration.serial_number = identity.serial_number;
    configuration.supports_remote_wakeup = true;
    configuration.max_power = MAX_POWER_MILLIAMPERES;
    configuration
}

fn gamepad_interface<'d, D: Driver<'d>>(
    builder: &mut Builder<'d, D>,
    control: &mut Control<'d>,
    face: Face,
) -> Gamepad<'d, D> {
    match face.mode {
        Mode::Xinput => {
            let mut function = builder.function(0, 0, 0);
            let mut interface = function.interface();
            let mut alternate = interface.alt_setting(
                xinput::INTERFACE_CLASS,
                xinput::INTERFACE_SUBCLASS,
                xinput::INTERFACE_PROTOCOL,
                None,
            );
            let input_address = EndpointAddress::from_parts(GAMEPAD_ENDPOINT, Direction::In);
            let output_address = EndpointAddress::from_parts(GAMEPAD_ENDPOINT, Direction::Out);
            alternate.descriptor(
                xinput::CLASS_DESCRIPTOR_TYPE,
                &xinput::class_descriptor(input_address.into(), output_address.into()),
            );
            let interval = face.gamepad_interval();
            let input = alternate.endpoint_interrupt_in(
                Some(input_address),
                xinput::MAX_PACKET_SIZE,
                interval,
            );
            let output = alternate.endpoint_interrupt_out(
                Some(output_address),
                xinput::MAX_PACKET_SIZE,
                interval,
            );
            Gamepad::Xinput { input, output }
        }
        Mode::Gamepad => Gamepad::Hid(hid_input_interface(
            builder,
            control,
            GAMEPAD_INTERFACE,
            hid::GAMEPAD_DESCRIPTOR,
            GAMEPAD_ENDPOINT,
            GAMEPAD_MAX_PACKET_SIZE,
            face.gamepad_interval(),
        )),
        Mode::NoGamepad => Gamepad::Placeholder(hid_input_interface(
            builder,
            control,
            GAMEPAD_INTERFACE,
            hid::PLACEHOLDER_DESCRIPTOR,
            GAMEPAD_ENDPOINT,
            max_packet_size(hid::PLACEHOLDER_LEN),
            PLACEHOLDER_INTERVAL,
        )),
    }
}

fn keyboard_interface<'d, D: Driver<'d>>(
    builder: &mut Builder<'d, D>,
    control: &mut Control<'d>,
    format: KeyboardFormat,
) -> D::EndpointIn {
    let (report_descriptor, report_len, interval) = match format {
        KeyboardFormat::NKeyRollover => (
            hid::NKRO_DESCRIPTOR,
            <hid::NkroReport as hid::InputReport>::LEN,
            NKRO_INTERVAL,
        ),
        KeyboardFormat::Boot => (
            hid::BOOT_KEYBOARD_DESCRIPTOR,
            <hid::BootReport as hid::InputReport>::LEN,
            BOOT_KEYBOARD_INTERVAL,
        ),
    };
    control.add_hid_interface(KEYBOARD_INTERFACE, report_descriptor);
    let mut function = builder.function(0, 0, 0);
    let mut interface = function.interface();
    let mut alternate = interface.alt_setting(
        hid_class::CLASS,
        hid_class::BOOT_SUBCLASS,
        hid_class::KEYBOARD_PROTOCOL,
        None,
    );
    alternate.descriptor(
        hid_class::DESCRIPTOR_TYPE,
        &hid_class::descriptor_body(report_descriptor),
    );
    alternate.endpoint_interrupt_in(
        Some(EndpointAddress::from_parts(
            KEYBOARD_ENDPOINT,
            Direction::In,
        )),
        max_packet_size(report_len),
        interval,
    )
}

fn channel_interface<'d, D: Driver<'d>>(
    builder: &mut Builder<'d, D>,
    control: &mut Control<'d>,
) -> ChannelEndpoints<'d, D> {
    control.add_hid_interface(CHANNEL_INTERFACE, hid::CHANNEL_DESCRIPTOR);
    let mut function = builder.function(0, 0, 0);
    let mut interface = function.interface();
    let mut alternate = interface.alt_setting(hid_class::CLASS, 0, 0, None);
    alternate.descriptor(
        hid_class::DESCRIPTOR_TYPE,
        &hid_class::descriptor_body(hid::CHANNEL_DESCRIPTOR),
    );
    let output = alternate.endpoint_interrupt_out(
        Some(EndpointAddress::from_parts(
            CHANNEL_ENDPOINT,
            Direction::Out,
        )),
        CHANNEL_MAX_PACKET_SIZE,
        CHANNEL_INTERVAL,
    );
    let input = alternate.endpoint_interrupt_in(
        Some(EndpointAddress::from_parts(CHANNEL_ENDPOINT, Direction::In)),
        CHANNEL_MAX_PACKET_SIZE,
        CHANNEL_INTERVAL,
    );
    ChannelEndpoints { output, input }
}

fn hid_input_interface<'d, D: Driver<'d>>(
    builder: &mut Builder<'d, D>,
    control: &mut Control<'d>,
    number: usize,
    report_descriptor: &'static [u8],
    endpoint: usize,
    max_packet_size: u16,
    interval: u8,
) -> D::EndpointIn {
    control.add_hid_interface(number, report_descriptor);
    let mut function = builder.function(0, 0, 0);
    let mut interface = function.interface();
    let mut alternate = interface.alt_setting(hid_class::CLASS, 0, 0, None);
    alternate.descriptor(
        hid_class::DESCRIPTOR_TYPE,
        &hid_class::descriptor_body(report_descriptor),
    );
    alternate.endpoint_interrupt_in(
        Some(EndpointAddress::from_parts(endpoint, Direction::In)),
        max_packet_size,
        interval,
    )
}

fn max_packet_size(report_len: usize) -> u16 {
    u16::try_from(report_len).unwrap_or(u16::MAX)
}
