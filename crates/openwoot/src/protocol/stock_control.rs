use openwoot_protocol::{
    lighting::KeyColors,
    report::{self, Contents},
    stock::{Command, MAGIC, Request, Status},
};

use crate::{
    keys::Key,
    lighting::{BLACK, HostControl, from_rgb565, rgb},
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
        lighting.set_colors(|keys| fill(keys, colors, matrix)).await;
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

fn fill(keys: &mut KeyColors, colors: &[u8], matrix: KeyMatrix) {
    keys.clear();
    let (rows, _) = colors.as_chunks::<{ COLOR_MAP_COLUMNS * COLOR_LEN }>();
    for row in 0..usize::from(matrix.rows) {
        let columns = rows
            .get(row)
            .map_or_default(|cells| cells.as_chunks::<COLOR_LEN>().0);
        for column in 0..usize::from(matrix.columns) {
            let color = columns
                .get(column)
                .map_or(BLACK, |&color| from_rgb565(u16::from_le_bytes(color)));
            if keys.push(color).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered() -> [u8; COLOR_MAP_LEN] {
        let mut colors = [0; COLOR_MAP_LEN];
        let (cells, _) = colors.as_chunks_mut::<COLOR_LEN>();
        for (number, cell) in (1_u16..).zip(cells) {
            *cell = number.to_le_bytes();
        }
        colors
    }

    fn numbered_color(row: usize, column: usize) -> u16 {
        u16::try_from(row * COLOR_MAP_COLUMNS + column + 1).unwrap()
    }

    #[test]
    fn a_matrix_inside_the_stock_map_takes_its_columns_of_every_row() {
        let mut keys = KeyColors::new();
        let matrix = KeyMatrix {
            rows: 6,
            columns: 17,
        };
        fill(&mut keys, &numbered(), matrix);
        assert_eq!(keys.len(), 6 * 17);
        for row in 0..6 {
            for column in 0..17 {
                assert_eq!(
                    keys[row * 17 + column],
                    from_rgb565(numbered_color(row, column)),
                    "row {row} column {column}"
                );
            }
        }
    }

    #[test]
    fn a_matrix_beyond_the_stock_map_keeps_its_rows_aligned_and_the_rest_unlit() {
        let mut keys = KeyColors::new();
        let wide = KeyMatrix {
            rows: 5,
            columns: 23,
        };
        fill(&mut keys, &numbered(), wide);
        assert_eq!(keys.len(), 5 * 23);
        for row in 0..5 {
            for column in 0..23 {
                let expected = if column < COLOR_MAP_COLUMNS {
                    from_rgb565(numbered_color(row, column))
                } else {
                    BLACK
                };
                assert_eq!(
                    keys[row * 23 + column],
                    expected,
                    "row {row} column {column}"
                );
            }
        }
        let tall = KeyMatrix {
            rows: 7,
            columns: 17,
        };
        fill(&mut keys, &numbered(), tall);
        assert_eq!(keys.len(), 7 * 17);
        assert_eq!(keys[5 * 17], from_rgb565(numbered_color(5, 0)));
        assert!(keys[6 * 17..].iter().all(|&color| color == BLACK));
    }
}
