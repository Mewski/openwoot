use embassy_futures::{
    join::{join, join4},
    select::{Either, select},
};
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_usb::driver::{Driver, EndpointIn, EndpointOut};

use super::{KeyReports, key_reports::ControlsReport};
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
        let keyboard_format = match self.host.keyboard_protocol() {
            HidProtocol::Boot => KeyboardFormat::Boot,
            HidProtocol::Report => self.face.keyboard,
        };
        reports.resend();
        let Self {
            host,
            gamepad,
            keyboard,
            controls,
            key_states,
            ..
        } = self;
        let (never, ..) = join4(
            forward_keyboard(reports, keyboard, keyboard_format),
            forward_controls(reports, controls),
            forward_key_states(reports, key_states),
            forward_gamepad(reports, gamepad, host),
        )
        .await;
        never
    }
}

async fn forward_keyboard<M: RawMutex>(
    reports: &KeyReports<M>,
    endpoint: &mut impl EndpointIn,
    format: KeyboardFormat,
) -> ! {
    let _ = match format {
        KeyboardFormat::NKeyRollover => write_report(endpoint, &NkroReport::new()).await,
        KeyboardFormat::Boot => write_report(endpoint, &BootReport::default()).await,
    };
    loop {
        let _ = match reports.keyboard().await {
            Keyboard::NKeyRollover(report) => write_report(endpoint, &report).await,
            Keyboard::Boot(report) => write_report(endpoint, &report).await,
        };
    }
}

async fn forward_controls<M: RawMutex>(
    reports: &KeyReports<M>,
    endpoint: &mut impl EndpointIn,
) -> ! {
    let _ = write_report(endpoint, &SystemReport::default()).await;
    let _ = write_report(endpoint, &ConsumerReport::default()).await;
    let _ = write_report(endpoint, &MouseReport::default()).await;
    loop {
        let _ = match reports.controls().await {
            ControlsReport::System(report) => write_report(endpoint, &report).await,
            ControlsReport::Consumer(report) => write_report(endpoint, &report).await,
            ControlsReport::Mouse(report) => write_report(endpoint, &report).await,
        };
    }
}

async fn forward_key_states<M: RawMutex>(
    reports: &KeyReports<M>,
    endpoint: &mut impl EndpointIn,
) -> ! {
    let _ = write_report(endpoint, &KeyStateReport::default()).await;
    loop {
        let report = reports.key_states().await;
        let _ = write_report(endpoint, &report).await;
    }
}

async fn forward_gamepad<'d, D: Driver<'d>, M: RawMutex>(
    reports: &KeyReports<M>,
    gamepad: &mut Gamepad<'d, D>,
    host: &HostState,
) -> ! {
    match gamepad {
        Gamepad::Xinput { input, output } => {
            let (never, ()) = join(forward_xinput(reports, input), listen(output, host)).await;
            never
        }
        Gamepad::Hid(input) => forward_hid_gamepad(reports, input).await,
        Gamepad::Placeholder(input) => {
            let _ = input.write_transfer(&[0; PLACEHOLDER_LEN], false).await;
            loop {
                reports.gamepad().await;
            }
        }
    }
}

async fn forward_xinput<M: RawMutex>(reports: &KeyReports<M>, endpoint: &mut impl EndpointIn) -> ! {
    let _ = write_report(endpoint, &xinput::Report::default()).await;
    loop {
        if let keys::GamepadReport::Xinput(report) = reports.gamepad().await {
            let _ = write_report(endpoint, &report).await;
        }
    }
}

async fn forward_hid_gamepad<M: RawMutex>(
    reports: &KeyReports<M>,
    endpoint: &mut impl EndpointIn,
) -> ! {
    let _ = write_report(endpoint, &GamepadReport::new()).await;
    loop {
        if let keys::GamepadReport::Hid(report) = reports.gamepad().await {
            let _ = write_report(endpoint, &report).await;
        }
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
