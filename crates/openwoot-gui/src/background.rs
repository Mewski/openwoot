mod launch;
mod settings;
mod tray;

use std::rc::Rc;

use async_channel::Sender;
use gpui_kit::{
    ActivationPolicy, AnyWindowHandle, App, AppContext as _, Context, DevicePixels, Entity, Global,
    Hsla, QuitMode, SharedString, SvgSize, Task, assets::IconName as Catalog,
    component::ActiveTheme as _, size,
};
use openwoot_protocol::profile::Selection;

pub(crate) use self::launch::HIDDEN_FLAG;
use self::{
    settings::Settings,
    tray::{Picture, Tray, TrayAction},
};
use crate::{app_watcher::AppWatcher, store::Store};

const ICON_SIDE: u32 = 64;
const TEMPLATE_COLOUR: &str = "#000000";
const CURRENT_COLOUR: &str = "currentColor";

pub(crate) type WindowOpener = Rc<dyn Fn(&mut App) -> Option<AnyWindowHandle>>;

pub(crate) struct Background {
    store: Entity<Store>,
    settings: Settings,
    starts_with_computer: bool,
    watcher: Option<Entity<AppWatcher>>,
    tray: Option<Tray>,
    window: Option<AnyWindowHandle>,
    open_window: WindowOpener,
    actions: Sender<TrayAction>,
    problem: Option<SharedString>,
    _listener: Task<()>,
}

struct Service(Entity<Background>);

impl Global for Service {}

impl Background {
    pub(crate) fn start(
        store: Entity<Store>,
        open_window: WindowOpener,
        cx: &mut App,
    ) -> Entity<Self> {
        let background = cx.new(|cx| Self::new(store, open_window, cx));
        cx.set_global(Service(background.clone()));
        cx.on_window_closed(|cx, _| {
            if let Some(background) = Self::global(cx) {
                background.update(cx, Self::window_closed);
            }
        })
        .detach();
        background
    }

    pub(crate) fn global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<Service>().map(|service| service.0.clone())
    }

    pub(crate) const fn is_running(&self) -> bool {
        self.settings.running
    }

    pub(crate) const fn starts_with_computer(&self) -> bool {
        self.starts_with_computer
    }

    pub(crate) const fn problem(&self) -> Option<&SharedString> {
        self.problem.as_ref()
    }

    pub(crate) fn set_running(&mut self, running: bool, cx: &mut Context<Self>) {
        self.settings.running = running;
        if !running {
            self.clear_selection(cx);
        }
        self.apply(cx);
        self.save(cx);
    }

    pub(crate) fn set_start_with_computer(&mut self, on: bool, cx: &mut Context<Self>) {
        match launch::set_start_with_computer(on) {
            Ok(()) => {
                self.starts_with_computer = on;
                self.problem = None;
            }
            Err(error) => self.problem = Some(described(error)),
        }
        cx.notify();
    }

    pub(crate) fn show(&mut self, cx: &mut Context<Self>) {
        cx.set_activation_policy(ActivationPolicy::Regular);
        cx.activate(true);
        let shown = self.window.is_some_and(|window| {
            window
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        });
        if !shown {
            self.window = (self.open_window)(cx);
        }
    }

    fn new(store: Entity<Store>, open_window: WindowOpener, cx: &mut Context<Self>) -> Self {
        let (actions, chosen) = async_channel::unbounded();
        let listener = cx.spawn(async move |this, cx| {
            while let Ok(action) = chosen.recv().await {
                if this
                    .update(cx, |background, cx| background.act(action, cx))
                    .is_err()
                {
                    return;
                }
            }
        });
        let mut background = Self {
            store,
            settings: Settings::load(),
            starts_with_computer: launch::starts_with_computer(),
            watcher: None,
            tray: None,
            window: None,
            open_window,
            actions,
            problem: None,
            _listener: listener,
        };
        background.apply(cx);
        background
    }

    fn act(&mut self, action: TrayAction, cx: &mut Context<Self>) {
        match action {
            TrayAction::Show => self.show(cx),
            TrayAction::Quit => cx.quit(),
        }
    }

    fn window_closed(&mut self, cx: &mut Context<Self>) {
        let windows = cx.windows();
        if self.window.is_some_and(|window| !windows.contains(&window)) {
            self.window = None;
        }
        if self.window.is_none() && self.settings.running {
            cx.set_activation_policy(ActivationPolicy::Accessory);
        }
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        if self.settings.running {
            cx.set_quit_mode(QuitMode::Explicit);
            if self.watcher.is_none() {
                let store = self.store.clone();
                self.watcher = Some(cx.new(|cx| AppWatcher::new(store, cx)));
            }
            if self.tray.is_none() {
                match tray_picture(cx).map(|picture| Tray::new(picture, self.actions.clone())) {
                    Some(Ok(tray)) => self.tray = Some(tray),
                    Some(Err(error)) => self.problem = Some(described(error)),
                    None => {}
                }
            }
        } else {
            cx.set_quit_mode(QuitMode::Default);
            self.watcher = None;
            self.tray = None;
        }
        cx.notify();
    }

    fn clear_selection(&self, cx: &mut Context<Self>) {
        let selected = self
            .store
            .read(cx)
            .keyboard()
            .is_some_and(|keyboard| keyboard.selected.is_some());
        if selected {
            let selection = Selection {
                profile: None,
                activate: false,
            };
            self.store
                .update(cx, |store, cx| store.select_linked(selection, cx))
                .detach();
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.settings.save() {
            self.problem = Some(described(error));
            cx.notify();
        }
    }
}

fn tray_picture(cx: &App) -> Option<Picture> {
    let colour = if cfg!(target_os = "macos") {
        TEMPLATE_COLOUR.to_owned()
    } else {
        hex(cx.theme().primary)
    };
    let bytes = cx.asset_source().load(&Catalog::Keyboard.path()).ok()??;
    let drawing = String::from_utf8_lossy(&bytes).replace(CURRENT_COLOUR, &colour);
    let renderer = cx.svg_renderer();
    let parsed = renderer.parse_svg(drawing.as_bytes()).ok()?;
    let side = DevicePixels(i32::try_from(ICON_SIDE).ok()?);
    let image = renderer
        .render_parsed(&parsed, SvgSize::ExactSize(size(side, side)))
        .ok()?;
    let rgba = image
        .as_bytes(0)?
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[blue, green, red, alpha]| [red, green, blue, alpha])
        .collect();
    Some(Picture {
        rgba,
        side: ICON_SIDE,
    })
}

fn hex(colour: Hsla) -> String {
    let rgba = colour.to_rgb();
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b)
    )
}

fn channel(value: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the channel is clamped to 0..=255 before the conversion"
    )]
    let channel = (value.clamp(0.0, 1.0) * f32::from(u8::MAX)).round() as u8;
    channel
}

fn described(error: impl std::error::Error + Send + Sync + 'static) -> SharedString {
    let text = format!("{:#}", anyhow::Error::new(error));
    let mut characters = text.chars();
    characters
        .next()
        .map_or_else(String::new, |first| {
            first.to_uppercase().chain(characters).collect()
        })
        .into()
}
