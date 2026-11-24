mod assets;
mod connection;
mod device;
mod keyboard;
mod page;
mod panel;
mod shell;
mod store;
mod theme;
mod usage;

use std::{
    io::{self, Write as _},
    process::ExitCode,
};

use gpui_kit::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

use crate::{device::Device, shell::Shell, store::Store};

const PROGRAM: &str = "openwoot-gui";
const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 860.0;

fn main() -> ExitCode {
    let (device, presence) = match Device::start() {
        Ok(started) => started,
        Err(error) => {
            writeln!(io::stderr().lock(), "{PROGRAM}: {error}").ok();
            return ExitCode::FAILURE;
        }
    };
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::apply(cx);
            let store = cx.new(|cx| Store::new(device, presence, cx));
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("openwoot".into()),
                    ..TitlebarOptions::default()
                }),
                ..WindowOptions::default()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| Shell::new(store, window, cx))
            }) {
                writeln!(
                    io::stderr().lock(),
                    "{PROGRAM}: could not open the window: {error:#}"
                )
                .ok();
                cx.quit();
            }
        });
    ExitCode::SUCCESS
}
