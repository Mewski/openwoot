use embassy_futures::{
    join::{join, join4},
    select::{Either, select},
};
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_usb::driver::{Driver, EndpointIn, EndpointOut};

use super::{KeyReports, Pending};
use crate::{
    hid::{
        BootReport, ConsumerReport, GamepadReport, KeyStateReport, MouseReport, NkroReport,
        PLACEHOLDER_LEN, SystemReport,
    },
    keys::{self, Keyboard},
    usb::{Face, Gamepad, HidProtocol, HostState, KeyboardFormat, write_report},
    xinput::{self, LedPattern},
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
    pub async fn run<M: RawMutex>(mut self, reports: &KeyReports<M>) -> ! {
        let host = self.host;
        loop {
            let mut is_configured = host.configuration_change().await;
            while is_configured {
                is_configured = match select(self.serve(reports), host.configuration_change()).await
                {
                    Either::First(never) => never,
                    Either::Second(next) => next,
                };
            }
        }
    }

    async fn serve<M: RawMutex>(&mut self, reports: &KeyReports<M>) -> ! {
        self.send_idle().await;
        reports.resend();
        let Self {
            host,
            gamepad,
            keyboard,
            controls,
            key_states,
            ..
        } = self;
        match gamepad {
            Gamepad::Xinput { input, output } => {
                let (never, ()) = join(
                    forward::<D, M>(
                        reports,
                        keyboard,
                        controls,
                        key_states,
                        GamepadIn::Xinput(input),
                    ),
                    listen(output, host),
                )
                .await;
                never
            }
            Gamepad::Hid(input) => {
                forward::<D, M>(
                    reports,
                    keyboard,
                    controls,
                    key_states,
                    GamepadIn::Hid(input),
                )
                .await
            }
            Gamepad::Placeholder(_) => {
                forward::<D, M>(reports, keyboard, controls, key_states, GamepadIn::None).await
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

enum GamepadIn<'e, E> {
    Xinput(&'e mut E),
    Hid(&'e mut E),
    None,
}

async fn forward<'d, D: Driver<'d>, M: RawMutex>(
    reports: &KeyReports<M>,
    keyboard: &mut D::EndpointIn,
    controls: &mut D::EndpointIn,
    key_states: &mut D::EndpointIn,
    mut gamepad: GamepadIn<'_, D::EndpointIn>,
) -> ! {
    loop {
        let Pending {
            keyboard: keyboard_report,
            system,
            consumer,
            mouse,
            key_states: key_state_report,
            gamepad: gamepad_report,
        } = reports.ready().await;
        let keyboard_write = async {
            let _ = match keyboard_report {
                Some(Keyboard::NKeyRollover(report)) => write_report(keyboard, &report).await,
                Some(Keyboard::Boot(report)) => write_report(keyboard, &report).await,
                None => Ok(()),
            };
        };
        let controls_write = async {
            if let Some(report) = system {
                let _ = write_report(controls, &report).await;
            }
            if let Some(report) = consumer {
                let _ = write_report(controls, &report).await;
            }
            if let Some(report) = mouse {
                let _ = write_report(controls, &report).await;
            }
        };
        let key_states_write = async {
            if let Some(report) = key_state_report {
                let _ = write_report(key_states, &report).await;
            }
        };
        let gamepad_write = async {
            let _ = match (&mut gamepad, gamepad_report) {
                (GamepadIn::Xinput(input), Some(keys::GamepadReport::Xinput(report))) => {
                    write_report(*input, &report).await
                }
                (GamepadIn::Hid(input), Some(keys::GamepadReport::Hid(report))) => {
                    write_report(*input, &report).await
                }
                _ => Ok(()),
            };
        };
        join4(
            keyboard_write,
            controls_write,
            key_states_write,
            gamepad_write,
        )
        .await;
    }
}

async fn listen(output: &mut impl EndpointOut, host: &HostState) {
    let mut packet = [0; XINPUT_OUTPUT_CAPACITY];
    while let Ok(length) = output.read(&mut packet).await {
        if let Some(player) = packet
            .get(..length)
            .and_then(LedPattern::from_packet)
            .and_then(LedPattern::player)
        {
            host.set_xinput_player(player);
        }
    }
}

async fn send_idle_gamepad<'d, D: Driver<'d>>(gamepad: &mut Gamepad<'d, D>) {
    match gamepad {
        Gamepad::Xinput { input, .. } => {
            let _ = write_report(input, &xinput::Report::default()).await;
        }
        Gamepad::Hid(input) => {
            let _ = write_report(input, &GamepadReport::new()).await;
        }
        Gamepad::Placeholder(input) => {
            let _ = input.write_transfer(&[0; PLACEHOLDER_LEN], false).await;
        }
    }
}
