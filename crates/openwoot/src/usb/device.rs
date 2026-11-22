use embassy_usb::{
    Builder, Config, UsbDevice, UsbVersion,
    driver::{Direction, Driver, EndpointAddress},
};
use openwoot_image::UsbIds;

use super::{
    Control, Face, HostState, KeyboardFormat, Mode,
    control::{HID_DESCRIPTOR_TYPE, KEYBOARD_INTERFACE, hid_class_descriptor},
    xinput,
};
use crate::hid;

const CONFIG_DESCRIPTOR_LEN: usize = 256;
const BOS_DESCRIPTOR_LEN: usize = 16;
const CONTROL_BUFFER_LEN: usize = 128;
const MAX_POWER_MILLIAMPERES: u16 = 400;
const HID_CLASS: u8 = 0x03;
const BOOT_SUBCLASS: u8 = 0x01;
const KEYBOARD_PROTOCOL: u8 = 0x01;
const GAMEPAD_INTERFACE: usize = 0;
const CHANNEL_INTERFACE: usize = 2;
const CONTROLS_INTERFACE: usize = 3;
const KEY_STATE_INTERFACE: usize = 4;
const GAMEPAD_ENDPOINT: usize = 1;
const KEYBOARD_ENDPOINT: usize = 2;
const CHANNEL_ENDPOINT: usize = 3;
const CONTROLS_ENDPOINT: usize = 4;
const KEY_STATE_ENDPOINT: usize = 5;
const GAMEPAD_MAX_PACKET_SIZE: u16 = 32;
const PLACEHOLDER_INTERVAL: u8 = 10;
const NKRO_INTERVAL: u8 = 1;
const BOOT_KEYBOARD_INTERVAL: u8 = 10;
const CHANNEL_MAX_PACKET_SIZE: u16 = 64;
const CHANNEL_INTERVAL: u8 = 1;
const CONTROLS_MAX_PACKET_SIZE: u16 = 8;
const CONTROLS_INTERVAL: u8 = 4;
const KEY_STATE_INTERVAL: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity<'a> {
    pub ids: UsbIds,
    pub manufacturer: &'a str,
    pub product: &'a str,
    pub serial_number: Option<&'a str>,
}

pub struct Resources<'d> {
    config_descriptor: [u8; CONFIG_DESCRIPTOR_LEN],
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
            config_descriptor: [0; CONFIG_DESCRIPTOR_LEN],
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
        config_descriptor,
        bos_descriptor,
        control_buffer,
        control,
    } = resources;
    let control = control.insert(Control::new(host, face.mode == Mode::Xinput));
    let mut builder = Builder::new(
        driver,
        device_config(face, identity),
        config_descriptor,
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

fn device_config(face: Face, identity: Identity<'_>) -> Config<'_> {
    let mut config = Config::new(
        identity.ids.vendor_id,
        identity.ids.product_id_base + face.mode.product_id_offset(),
    );
    config.bcd_usb = UsbVersion::Two;
    config.device_class = 0;
    config.device_sub_class = 0;
    config.device_protocol = 0;
    config.composite_with_iads = false;
    config.device_release = face.keyboard.device_release();
    config.manufacturer = Some(identity.manufacturer);
    config.product = Some(identity.product);
    config.serial_number = identity.serial_number;
    config.supports_remote_wakeup = true;
    config.max_power = MAX_POWER_MILLIAMPERES;
    config
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
    let mut alternate = interface.alt_setting(HID_CLASS, BOOT_SUBCLASS, KEYBOARD_PROTOCOL, None);
    alternate.descriptor(
        HID_DESCRIPTOR_TYPE,
        &hid_class_descriptor(report_descriptor),
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
    let mut alternate = interface.alt_setting(HID_CLASS, 0, 0, None);
    alternate.descriptor(
        HID_DESCRIPTOR_TYPE,
        &hid_class_descriptor(hid::CHANNEL_DESCRIPTOR),
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
    let mut alternate = interface.alt_setting(HID_CLASS, 0, 0, None);
    alternate.descriptor(
        HID_DESCRIPTOR_TYPE,
        &hid_class_descriptor(report_descriptor),
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
