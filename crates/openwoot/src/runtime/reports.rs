use embassy_futures::{
    join::{join, join4},
    select::{Either, select},
};
use embassy_usb::driver::{Driver, EndpointIn, EndpointOut};

use crate::{
    hid::{
        BootReport, ConsumerReport, GamepadReport, KeyStateReport, MouseReport, NkroReport,
        PLACEHOLDER_LEN, SystemReport,
    },
    usb::{Face, Gamepad, HidProtocol, HostState, KeyboardFormat, write_report},
    xinput,
};

const XINPUT_OUTPUT_CAPACITY: usize = 64;

pub struct Reports<'d, D: Driver<'d>> {
    pub(super) face: Face,
    pub(super) host: &'d HostState,
    pub(super) gamepad: Gamepad<'d, D>,
    pub(super) keyboard: D::EndpointIn,
    pub(super) controls: D::EndpointIn,
    pub(super) key_states: D::EndpointIn,
}

impl<'d, D: Driver<'d>> Reports<'d, D> {
    pub async fn run(mut self) -> ! {
        let host = self.host;
        loop {
            let mut is_configured = host.configuration_change().await;
            while is_configured {
                is_configured = match select(self.send_idle(), host.configuration_change()).await {
                    Either::First(()) => host.configuration_change().await,
                    Either::Second(next) => next,
                };
            }
        }
    }

    async fn send_idle(&mut self) {
        let keyboard_format = match self.host.keyboard_protocol() {
            HidProtocol::Boot => KeyboardFormat::Boot,
            HidProtocol::Report => self.face.keyboard,
        };
        let keyboard = async {
            let _ = match keyboard_format {
                KeyboardFormat::NKeyRollover => {
                    write_report(&mut self.keyboard, &NkroReport::new()).await
                }
                KeyboardFormat::Boot => {
                    write_report(&mut self.keyboard, &BootReport::default()).await
                }
            };
        };
        let controls = async {
            let _ = write_report(&mut self.controls, &SystemReport::default()).await;
            let _ = write_report(&mut self.controls, &ConsumerReport::default()).await;
            let _ = write_report(&mut self.controls, &MouseReport::default()).await;
        };
        let key_states = async {
            let _ = write_report(&mut self.key_states, &KeyStateReport::default()).await;
        };
        join4(
            send_idle_gamepad::<D>(&mut self.gamepad),
            keyboard,
            controls,
            key_states,
        )
        .await;
    }
}

async fn send_idle_gamepad<'d, D: Driver<'d>>(gamepad: &mut Gamepad<'d, D>) {
    match gamepad {
        Gamepad::Xinput { input, output } => {
            let mut packet = [0; XINPUT_OUTPUT_CAPACITY];
            let report = async {
                let _ = write_report(input, &xinput::Report::default()).await;
            };
            let drain = async { while output.read(&mut packet).await.is_ok() {} };
            join(report, drain).await;
        }
        Gamepad::Hid(input) => {
            let _ = write_report(input, &GamepadReport::new()).await;
        }
        Gamepad::Placeholder(input) => {
            let _ = input.write_transfer(&[0; PLACEHOLDER_LEN], false).await;
        }
    }
}
