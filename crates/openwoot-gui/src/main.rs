mod app_watcher;
mod assets;
mod background;
mod color_picker;
mod connection;
mod device;
mod dropdown;
mod eye_dropper;
mod history;
mod keyboard;
mod local_data;
mod page;
mod panel;
mod profile_parts;
mod shell;
mod slider;
mod store;
mod switch;
mod theme;
mod usage;

use std::{
    env,
    io::{self, Write as _},
    process::ExitCode,
    rc::Rc,
};

use gpui_kit::{
    ActivationPolicy, AnyWindowHandle, App, AppContext as _, Bounds, Entity, TitlebarOptions,
    WindowBounds, WindowOptions, px, size,
};

use crate::{
    background::{Background, HIDDEN_FLAG},
    device::Device,
    shell::Shell,
    store::Store,
};

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
    let hidden = env::args().any(|argument| argument == HIDDEN_FLAG);
    let application = gpui_kit::application().with_assets(assets::Assets);
    application.on_reopen(|cx| {
        if let Some(background) = Background::global(cx) {
            background.update(cx, Background::show);
        }
    });
    application.run(move |cx| {
        gpui_kit::init(cx);
        theme::apply(cx);
        shell::bind_keys(cx);
        let store = cx.new(|cx| Store::new(device, presence, cx));
        let opened = store.clone();
        let background = Background::start(store, Rc::new(move |cx| open_window(&opened, cx)), cx);
        if hidden && background.read(cx).is_running() {
            cx.set_activation_policy(ActivationPolicy::Accessory);
        } else {
            background.update(cx, Background::show);
        }
    });
    ExitCode::SUCCESS
}

fn open_window(store: &Entity<Store>, cx: &mut App) -> Option<AnyWindowHandle> {
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
    let store = store.clone();
    match gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| Shell::new(store, window, cx))
    }) {
        Ok((window, _)) => Some(window),
        Err(error) => {
            writeln!(
                io::stderr().lock(),
                "{PROGRAM}: could not open the window: {error:#}"
            )
            .ok();
            if cx.windows().is_empty() {
                cx.quit();
            }
            None
        }
    }
}
