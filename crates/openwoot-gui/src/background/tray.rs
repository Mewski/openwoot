use async_channel::Sender;
use tray_icon::{
    BadIcon, Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{self, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};

const SHOW: &str = "show";
const QUIT: &str = "quit";
const TOOLTIP: &str = "openwoot";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayAction {
    Show,
    Quit,
}

pub(crate) struct Tray {
    _icon: TrayIcon,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum TrayError {
    #[error("could not draw the tray icon")]
    Icon(#[source] BadIcon),
    #[error("could not build the tray menu")]
    Menu(#[source] menu::Error),
    #[error("could not show the tray icon")]
    Build(#[source] tray_icon::Error),
}

pub(crate) struct Picture {
    pub(crate) rgba: Vec<u8>,
    pub(crate) side: u32,
}

impl Tray {
    pub(crate) fn new(picture: Picture, actions: Sender<TrayAction>) -> Result<Self, TrayError> {
        let show = MenuItem::with_id(SHOW, "Show openwoot", true, None);
        let quit = MenuItem::with_id(QUIT, "Quit", true, None);
        let menu = Menu::new();
        menu.append_items(&[&show, &PredefinedMenuItem::separator(), &quit])
            .map_err(TrayError::Menu)?;
        let chosen = actions.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = if event.id == MenuId::new(SHOW) {
                TrayAction::Show
            } else if event.id == MenuId::new(QUIT) {
                TrayAction::Quit
            } else {
                return;
            };
            chosen.try_send(action).ok();
        }));
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if matches!(
                event,
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                actions.try_send(TrayAction::Show).ok();
            }
        }));
        let icon =
            Icon::from_rgba(picture.rgba, picture.side, picture.side).map_err(TrayError::Icon)?;
        let builder = with_picture(
            TrayIconBuilder::new()
                .with_tooltip(TOOLTIP)
                .with_menu(Box::new(menu)),
            icon,
        );
        let tray = builder.build().map_err(TrayError::Build)?;
        Ok(Self { _icon: tray })
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
        TrayIconEvent::set_event_handler(None::<fn(TrayIconEvent)>);
    }
}

#[cfg(target_os = "macos")]
fn with_picture(builder: TrayIconBuilder, icon: Icon) -> TrayIconBuilder {
    builder.with_icon_templated(icon)
}

#[cfg(not(target_os = "macos"))]
fn with_picture(builder: TrayIconBuilder, icon: Icon) -> TrayIconBuilder {
    builder.with_icon(icon)
}
