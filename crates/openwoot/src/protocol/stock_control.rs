use openwoot_protocol::{
    report::{self, Contents},
    stock::{Command, MAGIC, Request, Status},
};

use crate::{
    keys::Key,
    lighting::{HostControl, from_rgb565, rgb},
    usb::HostState,
};

const SET_KEY_COLORS: u8 = 11;
const COLOR_MAP_COLUMNS: usize = 21;
const COLOR_MAP_ROWS: usize = 6;
const COLOR_LEN: usize = 2;
const COLOR_MAP_LEN: usize = COLOR_MAP_ROWS * COLOR_MAP_COLUMNS * COLOR_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct KeyMatrix {
    pub(super) rows: u8,
    pub(super) columns: u8,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct StockControl<'r> {
    pub(super) host: Option<&'r HostState>,
    pub(super) lighting: Option<&'r HostControl>,
    pub(super) key_matrix: Option<KeyMatrix>,
}

impl StockControl<'_> {
    pub(super) async fn answer(&self, request: Request) -> Option<Status> {
        let [blue, green, red, key] = request.argument.to_le_bytes();
        match Command::from_number(request.command)? {
            Command::DisableKeyReports => self.with_host(|host| host.set_key_reports(false)),
            Command::EnableKeyReports => self.with_host(|host| host.set_key_reports(true)),
            Command::StartGamepadDetection => {
                self.with_host(|host| host.set_gamepad_detection(true))
            }
            Command::StopGamepadDetection => {
                self.with_host(|host| host.set_gamepad_detection(false))
            }
            Command::SetKeyColor => {
                let lighting = self.lighting?;
                lighting.enter().await;
                Some(match self.key(key) {
                    Some(key) => {
                        lighting.set_key_color(key, rgb(red, green, blue)).await;
                        Status::Success
                    }
                    None => Status::Error,
                })
            }
            Command::ResetKeyColor => {
                let lighting = self.lighting?;
                Some(match self.key(blue) {
                    Some(key) => {
                        lighting.reset_key_color(key).await;
                        Status::Success
                    }
                    None => Status::Error,
                })
            }
            Command::LeaveHostControl => {
                let lighting = self.lighting?;
                lighting.enter().await;
                lighting.leave().await;
                Some(Status::Success)
            }
            Command::EnterHostControl => {
                self.lighting?.enter().await;
                Some(Status::Success)
            }
            Command::Ping
            | Command::Version
            | Command::EnterUpdateMode
            | Command::Serial
            | Command::DeviceSettings => None,
        }
    }

    pub(super) async fn apply_output(&self, report: &[u8]) {
        let Ok(Contents::Foreign { payload, .. }) = report::parse(report) else {
            return;
        };
        let [first, second, SET_KEY_COLORS, colors @ ..] = payload else {
            return;
        };
        let (Some(lighting), Some(matrix), Some(colors)) =
            (self.lighting, self.key_matrix, colors.get(..COLOR_MAP_LEN))
        else {
            return;
        };
        if [*first, *second] != MAGIC {
            return;
        }
        lighting
            .set_colors(|keys| {
                keys.clear();
                let (rows, _) = colors.as_chunks::<{ COLOR_MAP_COLUMNS * COLOR_LEN }>();
                for row in rows.iter().take(usize::from(matrix.rows)) {
                    let (columns, _) = row.as_chunks::<COLOR_LEN>();
                    for &color in columns.iter().take(usize::from(matrix.columns)) {
                        if keys.push(from_rgb565(u16::from_le_bytes(color))).is_err() {
                            return;
                        }
                    }
                }
            })
            .await;
    }

    fn with_host(&self, change: impl FnOnce(&HostState)) -> Option<Status> {
        self.host.map(|host| {
            change(host);
            Status::Success
        })
    }

    fn key(&self, code: u8) -> Option<Key> {
        let key = Key::from_code(code)?;
        let matrix = self.key_matrix?;
        (key.row < matrix.rows && key.column < matrix.columns).then_some(key)
    }
}
