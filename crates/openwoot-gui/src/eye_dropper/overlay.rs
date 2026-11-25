mod desktop;
mod magnifier;
mod screen;

use std::mem;

use async_channel::Sender;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Div, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, ObjectFit, ParentElement as _, Pixels, Point, Render,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Task, Window, WindowBounds,
    WindowKind, WindowOptions,
    component::{ActiveTheme as _, h_flex},
    div, img,
};
use openwoot_protocol::lighting::Rgb;

use self::{magnifier::Magnifier, screen::Screen};
use crate::panel::ActionButton;

const ESCAPE: &str = "escape";

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("could not capture the screen: {0}")]
    Capture(#[from] xcap::XCapError),
    #[error("could not convert the screen colours: {0}")]
    Conversion(#[from] moxcms::CmsError),
    #[error("could not open the eye dropper: {0}")]
    Open(anyhow::Error),
    #[error("no display matches the captured screens")]
    Unmatched,
}

struct Session {
    windows: Vec<AnyWindowHandle>,
    picked: Sender<Option<Rgb>>,
}

struct Overlay {
    screen: Screen,
    session: Entity<Session>,
    pointer: Option<Point<Pixels>>,
    focus: FocusHandle,
    denied: Option<&'static str>,
}

pub(crate) fn pick(cx: &mut App) -> Task<Result<Option<Rgb>, Error>> {
    let capture = cx.background_spawn(async { Screen::capture() });
    cx.spawn(async move |cx| {
        let screens = capture.await?;
        let (sender, picked) = async_channel::bounded(1);
        cx.update(|cx| Session::open(screens, sender, cx))?;
        Ok(picked.recv().await.ok().flatten())
    })
}

impl Session {
    fn open(screens: Vec<Screen>, picked: Sender<Option<Rgb>>, cx: &mut App) -> Result<(), Error> {
        let session = cx.new(|_| Self {
            windows: Vec::new(),
            picked,
        });
        let displays = cx.displays();
        let mut windows = Vec::new();
        for screen in screens {
            let Some(display) = displays.iter().find(|display| screen.shows(display.id())) else {
                continue;
            };
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(display.bounds())),
                titlebar: None,
                focus: true,
                show: true,
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                display_id: Some(display.id()),
                ..WindowOptions::default()
            };
            let owner = session.clone();
            let opened = cx.open_window(options, |window, cx| {
                cx.new(|cx| Overlay::new(screen, owner, window, cx))
            });
            match opened {
                Ok(window) => windows.push(window.into()),
                Err(error) => {
                    close(windows, cx);
                    return Err(Error::Open(error));
                }
            }
        }
        if windows.is_empty() {
            return Err(Error::Unmatched);
        }
        session.update(cx, |session, _| session.windows = windows);
        Ok(())
    }

    fn finish(&mut self, picked: Option<Rgb>, cx: &mut App) {
        self.picked.try_send(picked).ok();
        let windows = mem::take(&mut self.windows);
        cx.defer(move |cx| close(windows, cx));
    }
}

impl Overlay {
    fn new(
        screen: Screen,
        session: Entity<Session>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        window.toggle_simple_fullscreen();
        Self {
            screen,
            session,
            pointer: None,
            focus,
            denied: desktop::denied(),
        }
    }

    fn finish(&self, picked: Option<Rgb>, cx: &mut App) {
        self.session
            .update(cx, |session, cx| session.finish(picked, cx));
    }

    fn pick_at(&self, position: Point<Pixels>, window: &Window, cx: &mut App) {
        let texel = self.screen.texel(position, window.viewport_size());
        self.finish(self.screen.color(texel), cx);
    }

    fn notice(&self, cx: &Context<Self>) -> Option<Div> {
        let settings = self.denied?;
        let theme = cx.theme();
        let session = self.session.clone();
        Some(
            h_flex()
                .absolute()
                .top_12()
                .left_0()
                .right_0()
                .justify_center()
                .child(
                    h_flex()
                        .gap_3()
                        .py_2()
                        .pl_4()
                        .pr_2()
                        .rounded(theme.radius_lg)
                        .bg(theme.popover)
                        .border_1()
                        .border_color(theme.border)
                        .shadow_lg()
                        .text_sm()
                        .text_color(theme.popover_foreground)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child("Allow screen recording to pick colours from other apps")
                        .child(
                            ActionButton::new("screen-recording", "Open settings")
                                .primary()
                                .on_click(move |_, _, cx| {
                                    session.update(cx, |session, cx| session.finish(None, cx));
                                    cx.open_url(settings);
                                }),
                        ),
                ),
        )
    }
}

impl Render for Overlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let magnifier = self
            .pointer
            .map(|pointer| Magnifier::new(&self.screen, pointer, window.viewport_size()));
        div()
            .id("eye-dropper")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .cursor_crosshair()
            .child(
                img(self.screen.image())
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            )
            .children(magnifier)
            .children(self.notice(cx))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !hovered {
                    this.pointer = None;
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.pointer = Some(event.position);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.pick_at(event.position, window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| this.finish(None, cx)),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == ESCAPE {
                    this.finish(None, cx);
                }
            }))
    }
}

fn close(windows: Vec<AnyWindowHandle>, cx: &mut App) {
    for window in windows {
        window
            .update(cx, |_, window, _| window.remove_window())
            .ok();
    }
}
