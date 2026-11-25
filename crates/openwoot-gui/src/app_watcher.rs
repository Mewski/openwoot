mod foreground;
mod matching;
mod steam;

use std::{
    collections::BTreeSet,
    fmt,
    io::{self, Write as _},
    time::Duration,
};

use gpui_kit::{AppContext as _, Context, Entity, SharedString, Subscription, Task};
use openwoot_protocol::profile::{Selection, Slot};

use self::matching::{linked_target, steam_ids, uses_title};
pub(crate) use self::{
    foreground::{Foreground, RunningApp, running_apps},
    matching::identity,
    steam::{InstalledGame, SteamApps, installed_games},
};
use crate::{
    device::Failure,
    store::{Store, StoreEvent},
};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const PROGRAM: &str = "openwoot-gui";

pub(crate) struct AppWatcher {
    store: Entity<Store>,
    steam: SteamApps,
    steam_ids: BTreeSet<u32>,
    requested: Request,
    _poll: Task<()>,
    _subscription: Subscription,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Request {
    Idle,
    Pending(Option<Slot>),
}

enum Decision {
    Select { app: String, profile: SharedString },
    Leave { app: String },
    Failed(Failure),
}

impl AppWatcher {
    pub(crate) fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                if this.update(cx, Self::check).is_err() {
                    return;
                }
            }
        });
        let subscription = cx.subscribe(&store, |this, _, event, _| {
            if matches!(event, StoreEvent::Connected | StoreEvent::Disconnected) {
                this.requested = Request::Idle;
            }
        });
        Self {
            store,
            steam: SteamApps::default(),
            steam_ids: BTreeSet::new(),
            requested: Request::Idle,
            _poll: poll,
            _subscription: subscription,
        }
    }

    fn check(&mut self, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let links = || keyboard.linked.iter().flat_map(|profile| &profile.links);
        if links().next().is_none() && keyboard.selected.is_none() {
            return;
        }
        let ids: BTreeSet<u32> = steam_ids(&keyboard.linked).collect();
        let titled = links().any(uses_title);
        let Some(foreground) = Foreground::current(titled) else {
            return;
        };
        if foreground.is_own() {
            return;
        }
        let target = linked_target(&keyboard.linked, &foreground, &self.steam);
        let selected = keyboard.selected;
        let decision = target.map_or_else(
            || Decision::Leave {
                app: foreground.name.clone(),
            },
            |slot| Decision::Select {
                app: foreground.name.clone(),
                profile: keyboard.profile_name(slot),
            },
        );
        if ids != self.steam_ids {
            self.locate_steam(ids, cx);
        }
        if target == selected {
            self.requested = Request::Idle;
            return;
        }
        if self.requested == Request::Pending(target) {
            return;
        }
        self.requested = Request::Pending(target);
        let selection = Selection {
            profile: target,
            activate: target.is_some(),
        };
        let request = self
            .store
            .update(cx, |store, cx| store.select_linked(selection, cx));
        cx.spawn(async move |_, _| {
            report(&match request.await {
                Ok(()) => decision,
                Err(error) => Decision::Failed(error),
            });
        })
        .detach();
    }

    fn locate_steam(&mut self, ids: BTreeSet<u32>, cx: &mut Context<Self>) {
        self.steam_ids.clone_from(&ids);
        let located = cx.background_spawn(async move { SteamApps::locate(ids) });
        cx.spawn(async move |this, cx| {
            let steam = located.await;
            this.update(cx, |watcher, _| watcher.steam = steam).ok();
        })
        .detach();
    }
}

impl fmt::Display for Decision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Select { app, profile } => write!(
                formatter,
                "{PROGRAM}: `{app}` is in front, selecting the linked profile `{profile}`"
            ),
            Self::Leave { app } => write!(
                formatter,
                "{PROGRAM}: `{app}` is in front, leaving the linked profile"
            ),
            Self::Failed(error) => write!(
                formatter,
                "{PROGRAM}: could not select the linked profile: {error:#}"
            ),
        }
    }
}

fn report(decision: &Decision) {
    writeln!(io::stderr().lock(), "{decision}").ok();
}
