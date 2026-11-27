mod apps;
mod card;
mod creator;
mod editor;
mod file;
mod icon;
mod library;
mod notice;
mod preset;
mod transfer;

use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
};

use gpui_kit::{
    Anchor, AnyElement, App, AppContext as _, AsyncApp, Context, Div, Entity, FontWeight,
    InteractiveElement as _, IntoElement, Modifiers, ParentElement as _, PathPromptOptions, Render,
    SharedString, Styled as _, Subscription, WeakEntity, Window, anchored,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, WindowExt as _, h_flex, notification::Notification,
        progress::Progress, v_flex,
    },
    deferred, div, point,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    apps::Link,
    endpoints::{
        GetGamepadMapping, GetKeyOverrides, GetKeySettings, GetKeymap, GetLighting,
        SetGamepadMapping, SetKeyOverrides, SetKeySettings, SetLighting, SetProfileInfo,
    },
    gamepad, heapless,
    keys::{self, Keymap, Layer},
    lighting,
    profile::{self, Change, LayerRef, NAME_LEN, Namespace, Slot},
};

pub(super) use self::card::count_chip;
pub(crate) use self::icon::avatar;
use self::{
    card::{Card, Face, Moving, Origin, Place, Status},
    creator::Creator,
    editor::{Editor, Linking},
    file::{EXTENSION, ProfileFile},
    library::Stored,
    preset::{Movement, Preset},
    transfer::{Onboard, Snapshot},
};
use super::Page;
use crate::{
    app_watcher,
    background::Background,
    dropdown::{Dropdown, Item},
    panel::{self, ActionButton, Heading},
    profile_parts::{apply, fetch},
    store::{Keyboard, Scope, Store, StoreEvent},
    usage,
};

type Name = heapless::String<NAME_LEN>;
type Outcome = Result<SharedString, String>;

const ONBOARD_HELP: &str =
    "Onboard profiles are stored on your device and can be accessed anytime.";
const INACTIVE_HELP: &str = "Inactive profiles are stored only in openwoot.";
const LINKED_HELP: &str = "Linked profiles are stored on your device and activate automatically when their linked application is in focus.";
const LINKED_SUBHEADER: &str =
    "Linked profiles activate automatically when their linked application is your active window.";
const LINKING_NEEDS_SERVICE: &str = "Background Service must be running for app linking to work.";
const HINT_BOTTOM: f32 = 24.0;
const HINT_PADDING_X: f32 = 16.0;
const HINT_PADDING_Y: f32 = 8.0;
const HINT_RADIUS: f32 = 8.0;
const HINT_TEXT: f32 = 14.0;
const HINT_LINE: f32 = 16.8;
const ACTION_GAP: f32 = 8.0;
const INTRO_GAP: f32 = 8.0;
const SECTION_GAP: f32 = 20.0;
const COPY_HINT: &str = "Hold Ctrl to copy instead of move";
const RELEASE_HINT: &str = "Release to copy profile";
const CREATED: &str = "The profile has been created.";
const LINKED: &str = "The profile has been linked.";

pub(crate) struct ProfilesPage {
    store: Entity<Store>,
    library: Result<PathBuf, SharedString>,
    inactive: Vec<Stored>,
    job: Option<Job>,
    creator: Option<Entity<Creator>>,
    dragging: Option<Origin>,
    _subscriptions: Vec<Subscription>,
}

struct Job {
    title: SharedString,
    done: usize,
    total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Subject {
    Onboard(Slot),
    Linked(Slot),
    Inactive(PathBuf),
    LinkOnboard(Slot),
    LinkInactive(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Edit,
    Duplicate,
    Move,
    Copy,
    LinkToApp,
    Unlink,
    SetDefault,
    Export,
    Delete,
}

impl Subject {
    const fn linked_slot(&self) -> Option<Slot> {
        match self {
            Self::Linked(slot) => Some(*slot),
            Self::Onboard(_) | Self::Inactive(_) | Self::LinkOnboard(_) | Self::LinkInactive(_) => {
                None
            }
        }
    }

    const fn is_linking(&self) -> bool {
        matches!(self, Self::LinkOnboard(_) | Self::LinkInactive(_))
    }
}

impl ProfilesPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, _, cx| {
                if matches!(event, StoreEvent::Disconnected) {
                    this.creator = None;
                    cx.notify();
                }
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
        ];
        subscriptions.extend(
            Background::global(cx)
                .map(|background| cx.observe(&background, |_, _, cx| cx.notify())),
        );
        let library = library::directory().map_err(|error| SharedString::from(error_chain(&error)));
        let page = Self {
            store,
            library,
            inactive: Vec::new(),
            job: None,
            creator: None,
            dragging: None,
            _subscriptions: subscriptions,
        };
        page.load_library(cx);
        page
    }

    fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn load_library(&self, cx: &mut Context<Self>) {
        let Ok(directory) = self.library.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let listed = library::on_worker(move || library::list(&directory)).await;
            this.update(cx, |page, cx| {
                match listed {
                    Ok(inactive) => page.inactive = inactive,
                    Err(error) => page.library = Err(error_chain(&error).into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn begin(
        &mut self,
        title: impl Into<SharedString>,
        total: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.busy() {
            return false;
        }
        self.job = Some(Job {
            title: title.into(),
            done: 0,
            total,
        });
        cx.notify();
        true
    }

    fn advance(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &mut self.job {
            job.done = (job.done + 1).min(job.total);
        }
        cx.notify();
    }

    fn finish(&mut self, outcome: Outcome, window: &mut Window, cx: &mut Context<Self>) {
        self.job = None;
        match outcome {
            Ok(message) => window.push_notification(Notification::success(message), cx),
            Err(problem) => window.push_notification(Notification::error(sentence(&problem)), cx),
        }
        self.load_library(cx);
        cx.notify();
    }

    fn activate(&self, slot: Slot, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.activate(slot, cx))
            .detach();
    }

    fn lift(&mut self, origin: Origin, cx: &mut Context<Self>) {
        self.dragging = Some(origin);
        cx.notify();
    }

    fn status(&self, origin: Origin, cx: &App) -> Status {
        if self.busy() {
            Status::Busy
        } else if cx.has_active_drag() && self.dragging == Some(origin) {
            Status::Lifted
        } else {
            Status::Ready
        }
    }

    fn command(
        &mut self,
        origin: Origin,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match (origin, command) {
            (Origin::Onboard(slot), Command::Edit) => self.edit_onboard(slot, window, cx),
            (Origin::Linked(slot), Command::Edit) => self.edit_linked(slot, window, cx),
            (Origin::Inactive(index), Command::Edit) => self.edit_inactive(index, window, cx),
            (Origin::Onboard(slot) | Origin::Linked(slot), Command::Duplicate) => {
                self.move_to_inactive(slot, true, window, cx);
            }
            (Origin::Inactive(index), Command::Duplicate) => {
                self.duplicate_inactive(index, window, cx);
            }
            (Origin::Onboard(slot), Command::Move) => {
                self.move_to_inactive(slot, false, window, cx);
            }
            (Origin::Inactive(index), Command::Move) => {
                self.move_to_onboard(index, false, window, cx);
            }
            (Origin::Inactive(index), Command::Copy) => {
                self.move_to_onboard(index, true, window, cx);
            }
            (Origin::Onboard(slot), Command::LinkToApp) => self.link_onboard(slot, window, cx),
            (Origin::Inactive(index), Command::LinkToApp) => {
                self.link_inactive(index, window, cx);
            }
            (Origin::Linked(slot), Command::Unlink) => self.unlink(slot, window, cx),
            (Origin::Onboard(slot), Command::SetDefault) => {
                self.swap(slot, Slot::new(0), window, cx);
            }
            (Origin::Inactive(index), Command::SetDefault) => {
                self.exchange(index, Slot::new(0), false, window, cx);
            }
            (Origin::Onboard(slot) | Origin::Linked(slot), Command::Export) => {
                self.export_onboard(slot, window, cx);
            }
            (Origin::Inactive(index), Command::Export) => self.export_inactive(index, window, cx),
            (origin, Command::Delete) => self.confirm_delete(origin, window, cx),
            (Origin::Onboard(_) | Origin::Linked(_), Command::Copy)
            | (Origin::Linked(_), Command::Move | Command::LinkToApp | Command::SetDefault)
            | (Origin::Onboard(_) | Origin::Inactive(_), Command::Unlink) => {}
        }
    }

    fn drop_profile(
        &mut self,
        moving: &Moving,
        place: Place,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let copy = copy_held(window.modifiers());
        match (moving.origin, place) {
            (Origin::Onboard(from), Place::Onboard(to)) if from != to => {
                self.swap(from, to, window, cx);
            }
            (Origin::Onboard(slot), Place::Inactive) => {
                self.move_to_inactive(slot, copy, window, cx);
            }
            (Origin::Inactive(index), Place::Onboard(slot)) => {
                self.exchange(index, slot, copy, window, cx);
            }
            (Origin::Inactive(index), Place::NewOnboard) => {
                self.move_to_onboard(index, copy, window, cx);
            }
            (Origin::Onboard(slot), Place::NewLinked) => self.link_onboard(slot, window, cx),
            (Origin::Inactive(index), Place::NewLinked) => self.link_inactive(index, window, cx),
            (Origin::Linked(slot), Place::Inactive) if copy => {
                self.move_to_inactive(slot, true, window, cx);
            }
            (Origin::Linked(slot), Place::Inactive) => self.unlink(slot, window, cx),
            (Origin::Linked(slot), Place::NewOnboard) => {
                self.move_linked_to_onboard(slot, copy, window, cx);
            }
            _ => {}
        }
    }

    fn edit_onboard(&self, slot: Slot, window: &mut Window, cx: &mut Context<Self>) {
        let Some(info) = self
            .store
            .read(cx)
            .keyboard()
            .and_then(|keyboard| keyboard.info(slot))
            .cloned()
        else {
            return;
        };
        Editor::open(
            cx.entity().downgrade(),
            Subject::Onboard(slot),
            &info,
            None,
            window,
            cx,
        );
    }

    fn edit_linked(&self, slot: Slot, window: &mut Window, cx: &mut Context<Self>) {
        let Some(linked) = self
            .store
            .read(cx)
            .keyboard()
            .and_then(|keyboard| keyboard.linked_profile(slot))
            .cloned()
        else {
            return;
        };
        let linking = Linking {
            store: self.store.clone(),
            links: linked.links,
        };
        Editor::open(
            cx.entity().downgrade(),
            Subject::Linked(slot),
            &linked.info,
            Some(linking),
            window,
            cx,
        );
    }

    fn edit_inactive(&self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(stored) = self.inactive.get(index) else {
            return;
        };
        let subject = Subject::Inactive(stored.path.clone());
        let info = stored.file.info.clone();
        Editor::open(cx.entity().downgrade(), subject, &info, None, window, cx);
    }

    fn link_onboard(&self, slot: Slot, window: &mut Window, cx: &mut Context<Self>) {
        let Some(info) = self
            .store
            .read(cx)
            .keyboard()
            .and_then(|keyboard| keyboard.info(slot))
            .cloned()
        else {
            return;
        };
        let linking = Linking {
            store: self.store.clone(),
            links: Vec::new(),
        };
        Editor::open(
            cx.entity().downgrade(),
            Subject::LinkOnboard(slot),
            &info,
            Some(linking),
            window,
            cx,
        );
    }

    fn link_inactive(&self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(stored) = self.inactive.get(index) else {
            return;
        };
        let linking = Linking {
            store: self.store.clone(),
            links: stored.file.links.clone(),
        };
        Editor::open(
            cx.entity().downgrade(),
            Subject::LinkInactive(stored.path.clone()),
            &stored.file.info,
            Some(linking),
            window,
            cx,
        );
    }

    fn apply_edit(
        &mut self,
        subject: Subject,
        info: profile::Info,
        links: Option<Vec<Link>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match subject {
            Subject::Onboard(slot) => {
                self.store
                    .update(cx, |store, cx| store.set_profile_info(slot, info, cx))
                    .detach();
            }
            Subject::Linked(slot) => {
                self.store
                    .update(cx, |store, cx| store.set_profile_info(slot, info, cx))
                    .detach();
                if let Some(links) = links {
                    self.replace_links(slot, links, cx);
                }
            }
            Subject::Inactive(path) => {
                let Some(mut stored) = self
                    .inactive
                    .iter()
                    .find(|stored| stored.path == path)
                    .cloned()
                else {
                    return;
                };
                stored.file.info = info;
                cx.spawn_in(window, async move |this, cx| {
                    let written = library::on_worker(move || library::update(&stored)).await;
                    this.update_in(cx, |page, window, cx| match written {
                        Ok(()) => page.load_library(cx),
                        Err(error) => page.finish(Err(error_chain(&error)), window, cx),
                    })
                    .ok();
                })
                .detach();
            }
            Subject::LinkOnboard(slot) => {
                self.move_onboard_to_linked(slot, info, links.unwrap_or_default(), window, cx);
            }
            Subject::LinkInactive(path) => {
                self.move_inactive_to_linked(&path, info, links.unwrap_or_default(), window, cx);
            }
        }
    }

    fn replace_links(&self, slot: Slot, links: Vec<Link>, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let changed = keyboard
            .linked_profile(slot)
            .is_none_or(|linked| linked.links != links);
        let elsewhere = links_elsewhere(keyboard, Some(slot), &links);
        self.store.update(cx, |store, cx| {
            for (other, kept) in elsewhere {
                store.set_app_links(other, kept, cx).detach();
            }
            if changed {
                store.set_app_links(slot, links, cx).detach();
            }
        });
    }

    fn linking_room(keyboard: &Keyboard) -> bool {
        keyboard.linked.len() < usize::from(keyboard.capabilities.linked_profiles)
    }

    fn move_onboard_to_linked(
        &mut self,
        slot: Slot,
        info: profile::Info,
        links: Vec<Link>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let onboard = onboard_of(keyboard);
        if !Self::linking_room(keyboard) || onboard.count <= 1 {
            return;
        }
        let elsewhere = links_elsewhere(keyboard, None, &links);
        let title = format!("Linking “{}”", info.name);
        let total = onboard.copy_steps()
            + onboard.insert_steps()
            + transfer::LINK_STEPS
            + transfer::REMOVE_STEPS;
        if !self.begin(title, total, cx) {
            return;
        }
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut step = progress(&this);
            let linking = Relinking {
                info,
                links,
                elsewhere,
            };
            let outcome = Box::pin(link_onboard_profile(
                &store, slot, onboard, linking, cx, &mut step,
            ))
            .await;
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn move_inactive_to_linked(
        &mut self,
        path: &Path,
        info: profile::Info,
        links: Vec<Link>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let Some(stored) = self
            .inactive
            .iter()
            .find(|stored| stored.path == path)
            .cloned()
        else {
            return;
        };
        if !Self::linking_room(keyboard) {
            return;
        }
        let mut snapshot = match snapshot_of(&stored, keyboard) {
            Ok(snapshot) => snapshot,
            Err(problem) => {
                window.push_notification(Notification::error(sentence(&problem)), cx);
                return;
            }
        };
        snapshot.info = info;
        let elsewhere = links_elsewhere(keyboard, None, &links);
        let onboard = onboard_of(keyboard);
        let title = format!("Linking “{}”", snapshot.info.name);
        if !self.begin(title, onboard.insert_steps() + transfer::LINK_STEPS + 1, cx) {
            return;
        }
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut step = progress(&this);
            let outcome = Box::pin(link_inactive_profile(
                &store, stored, snapshot, links, elsewhere, cx, &mut step,
            ))
            .await;
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn unlink(&mut self, slot: Slot, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let Ok(directory) = self.library.clone() else {
            return;
        };
        let Some(links) = keyboard
            .linked_profile(slot)
            .map(|linked| linked.links.clone())
        else {
            return;
        };
        let onboard = onboard_of(keyboard);
        let shifted = keyboard
            .linked
            .len()
            .saturating_sub(usize::from(slot.index()) + 1);
        let maker = keyboard.name.to_string();
        let title = format!(
            "Moving “{}” to inactive profiles",
            keyboard.profile_name(slot)
        );
        let total =
            onboard.copy_steps() + 1 + transfer::remove_linked_steps(onboard.lightbar, shifted);
        if !self.begin(title, total, cx) {
            return;
        }
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut step = progress(&this);
            let outcome = Box::pin(unlink_profile(
                &store, slot, directory, maker, links, onboard, cx, &mut step,
            ))
            .await;
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn move_linked_to_onboard(
        &mut self,
        slot: Slot,
        copy: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        if keyboard.profiles.len() >= usize::from(keyboard.capabilities.profiles) {
            window.push_notification(
                Notification::warning("Device has no free onboard profile slots"),
                cx,
            );
            return;
        }
        let onboard = onboard_of(keyboard);
        let shifted = keyboard
            .linked
            .len()
            .saturating_sub(usize::from(slot.index()) + 1);
        let title = format!(
            "Moving “{}” to onboard profiles",
            keyboard.profile_name(slot)
        );
        let removal = if copy {
            0
        } else {
            transfer::remove_linked_steps(onboard.lightbar, shifted)
        };
        if !self.begin(
            title,
            onboard.copy_steps() + onboard.insert_steps() + removal,
            cx,
        ) {
            return;
        }
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut step = progress(&this);
            let outcome = Box::pin(onboard_linked_profile(
                &store, slot, onboard, copy, cx, &mut step,
            ))
            .await;
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn move_to_inactive(
        &mut self,
        slot: Slot,
        copy: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let Ok(directory) = self.library.clone() else {
            return;
        };
        let onboard = onboard_of(keyboard);
        if !copy && onboard.count <= 1 {
            return;
        }
        let name = keyboard.profile_name(slot);
        let maker = keyboard.name.to_string();
        let (title, total) = if copy {
            (format!("Duplicating “{name}”"), onboard.copy_steps() + 1)
        } else {
            (
                format!("Moving “{name}” to inactive profiles"),
                onboard.copy_steps() + 1 + transfer::REMOVE_STEPS,
            )
        };
        if !self.begin(title, total, cx) {
            return;
        }
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut step = progress(&this);
            let outcome = Box::pin(save_inactive(
                &store, slot, directory, maker, onboard, copy, cx, &mut step,
            ))
            .await;
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn move_to_onboard(
        &mut self,
        index: usize,
        copy: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let Some(stored) = self.inactive.get(index).cloned() else {
            return;
        };
        if keyboard.profiles.len() >= usize::from(keyboard.capabilities.profiles) {
            window.push_notification(
                Notification::warning("Device has no free onboard profile slots"),
                cx,
            );
            return;
        }
        let snapshot = match snapshot_of(&stored, keyboard) {
            Ok(snapshot) => snapshot,
            Err(problem) => {
                window.push_notification(Notification::error(sentence(&problem)), cx);
                return;
            }
        };
        let onboard = onboard_of(keyboard);
        let title = format!("Loading “{}” onto the keyboard", stored.file.info.name);
        if !self.begin(title, onboard.insert_steps() + 1, cx) {
            return;
        }
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut step = progress(&this);
            let outcome =
                Box::pin(load_inactive(&store, stored, snapshot, copy, cx, &mut step)).await;
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn exchange(
        &mut self,
        index: usize,
        slot: Slot,
        copy: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let Ok(directory) = self.library.clone() else {
            return;
        };
        let Some(stored) = self.inactive.get(index).cloned() else {
            return;
        };
        let snapshot = match snapshot_of(&stored, keyboard) {
            Ok(snapshot) => snapshot,
            Err(problem) => {
                window.push_notification(Notification::error(sentence(&problem)), cx);
                return;
            }
        };
        let onboard = onboard_of(keyboard);
        let maker = keyboard.name.to_string();
        let title = format!(
            "Swapping “{}” with “{}”",
            stored.file.info.name,
            keyboard.profile_name(slot)
        );
        let total = onboard.copy_steps() + onboard.replace_steps() + 2;
        if !self.begin(title, total, cx) {
            return;
        }
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut step = progress(&this);
            let incoming = Incoming {
                stored,
                snapshot,
                copy,
            };
            let outcome = Box::pin(exchange(
                &store, incoming, slot, directory, maker, onboard, cx, &mut step,
            ))
            .await;
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn swap(&mut self, first: Slot, second: Slot, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let title = format!(
            "Swapping “{}” with “{}”",
            keyboard.profile_name(first),
            keyboard.profile_name(second)
        );
        if !self.begin(title, transfer::SWAP_STEPS, cx) {
            return;
        }
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut step = progress(&this);
            let outcome = Box::pin(transfer::swap(&store, first, second, cx, &mut step))
                .await
                .map(|()| SharedString::from("Profiles swapped"));
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn duplicate_inactive(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(directory) = self.library.clone() else {
            return;
        };
        let Some(stored) = self.inactive.get(index).cloned() else {
            return;
        };
        if !self.begin(format!("Duplicating “{}”", stored.file.info.name), 1, cx) {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            let added = library::on_worker(move || library::add(&directory, stored.file)).await;
            let outcome = added
                .map(|_| SharedString::from("Profile duplicated"))
                .map_err(|error| error_chain(&error));
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn confirm_delete(&self, origin: Origin, window: &mut Window, cx: &mut Context<Self>) {
        let name = match origin {
            Origin::Onboard(slot) | Origin::Linked(slot) => self
                .store
                .read(cx)
                .keyboard()
                .map(|keyboard| keyboard.profile_name(slot)),
            Origin::Inactive(index) => self
                .inactive
                .get(index)
                .map(|stored| stored.file.info.name.as_str().to_owned().into()),
        };
        let Some(name) = name else {
            return;
        };
        let page = cx.entity().downgrade();
        panel::confirm(
            panel::Confirmation::new(
                "Delete profile",
                format!("Are you sure you want to remove “{name}”?"),
                "Delete profile",
            )
            .danger(),
            move |window, cx| {
                page.update(cx, |page, cx| page.delete(origin, window, cx))
                    .ok();
            },
            window,
            cx,
        );
    }

    fn delete(&mut self, origin: Origin, window: &mut Window, cx: &mut Context<Self>) {
        match origin {
            Origin::Onboard(slot) => {
                let Some(keyboard) = self.store.read(cx).keyboard() else {
                    return;
                };
                let onboard = onboard_of(keyboard);
                let title = format!("Deleting “{}”", keyboard.profile_name(slot));
                if slot.get() == 0 || !self.begin(title, transfer::REMOVE_STEPS, cx) {
                    return;
                }
                let store = self.store.clone();
                cx.spawn_in(window, async move |this, cx| {
                    let mut step = progress(&this);
                    let outcome = Box::pin(transfer::remove(&store, slot, onboard, cx, &mut step))
                        .await
                        .map(|()| SharedString::from("The profile has been deleted."));
                    this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                        .ok();
                })
                .detach();
            }
            Origin::Linked(slot) => {
                let Some(keyboard) = self.store.read(cx).keyboard() else {
                    return;
                };
                let lightbar = keyboard.capabilities.lightbar_leds > 0;
                let shifted = keyboard
                    .linked
                    .len()
                    .saturating_sub(usize::from(slot.index()) + 1);
                let title = format!("Deleting “{}”", keyboard.profile_name(slot));
                let total = transfer::remove_linked_steps(lightbar, shifted);
                if !self.begin(title, total, cx) {
                    return;
                }
                let store = self.store.clone();
                cx.spawn_in(window, async move |this, cx| {
                    let mut step = progress(&this);
                    let outcome = Box::pin(transfer::remove_linked(
                        &store, slot, lightbar, cx, &mut step,
                    ))
                    .await
                    .map(|()| SharedString::from("The profile has been deleted."));
                    this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                        .ok();
                })
                .detach();
            }
            Origin::Inactive(index) => {
                let Some(stored) = self.inactive.get(index).cloned() else {
                    return;
                };
                if !self.begin(format!("Deleting “{}”", stored.file.info.name), 1, cx) {
                    return;
                }
                cx.spawn_in(window, async move |this, cx| {
                    let removed = library::on_worker(move || library::remove(&stored)).await;
                    let outcome = removed
                        .map(|()| SharedString::from("The profile has been deleted."))
                        .map_err(|error| error_chain(&error));
                    this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                        .ok();
                })
                .detach();
            }
        }
    }

    fn export_onboard(&mut self, slot: Slot, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let onboard = onboard_of(keyboard);
        let name = keyboard.profile_name(slot);
        let maker = keyboard.name.to_string();
        let chosen = cx.prompt_for_new_path(&home_directory(), Some(&suggested_file(&name)));
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let Some(path) = chosen_path(chosen.await) else {
                return;
            };
            let title = format!("Exporting “{name}”");
            if !this
                .update(cx, |page, cx| {
                    page.begin(title, onboard.copy_steps() + 1, cx)
                })
                .unwrap_or(false)
            {
                return;
            }
            let mut step = progress(&this);
            let outcome = Box::pin(export(&store, slot, maker, path, onboard, cx, &mut step)).await;
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn export_inactive(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(stored) = self.inactive.get(index).cloned() else {
            return;
        };
        let chosen = cx.prompt_for_new_path(
            &home_directory(),
            Some(&suggested_file(&stored.file.info.name)),
        );
        cx.spawn_in(window, async move |this, cx| {
            let Some(path) = chosen_path(chosen.await) else {
                return;
            };
            let target = path.clone();
            let saved = library::on_worker(move || stored.file.save(&target)).await;
            let outcome = saved
                .map(|()| {
                    SharedString::from(format!(
                        "The profile has been exported to {}.",
                        path.display()
                    ))
                })
                .map_err(|error| error_chain(&error));
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    pub(crate) fn choose_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Ok(directory) = self.library.clone() else {
            return;
        };
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import Profile".into()),
        });
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let path = match chosen.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Err(error)) => {
                    let problem = format!("could not open the file dialog: {error:#}");
                    this.update_in(cx, |page, window, cx| page.finish(Err(problem), window, cx))
                        .ok();
                    None
                }
                Ok(Ok(None)) | Err(_) => None,
            };
            let Some(path) = path else {
                return;
            };
            let keyboard = store.read_with(cx, |store, _| store.keyboard().cloned());
            let outcome = match keyboard {
                Some(keyboard) => Box::pin(import(directory, path, keyboard)).await,
                None => Err("Device must be connected to import a profile".to_owned()),
            };
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    pub(crate) fn open_creator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        let store = self.store.clone();
        let library = self.library.is_ok();
        self.creator = Some(cx.new(|cx| Creator::new(page, store, library, window, cx)));
        cx.notify();
    }

    fn close_creator(&mut self, cx: &mut Context<Self>) {
        self.creator = None;
        cx.notify();
    }

    fn create(
        &mut self,
        preset: Preset,
        info: profile::Info,
        destination: Destination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let onboard = onboard_of(keyboard);
        let maker = keyboard.name.to_string();
        let (namespace, library, links) = match destination {
            Destination::Onboard => (Namespace::Onboard, None, None),
            Destination::Linked(links) => (Namespace::Linked, None, Some(links)),
            Destination::Inactive => match self.library.clone() {
                Ok(directory) => (Namespace::Onboard, Some(directory), None),
                Err(_) => return,
            },
        };
        let elsewhere = links
            .as_ref()
            .map_or_else(Vec::new, |links| links_elsewhere(keyboard, None, links));
        let built = Onboard {
            count: onboard.count.saturating_add(1),
            ..onboard
        };
        let total = preset_steps(preset)
            + library
                .as_ref()
                .map_or(0, |_| built.copy_steps() + 1 + transfer::REMOVE_STEPS)
            + links.as_ref().map_or(0, |_| transfer::LINK_STEPS);
        if !self.begin(format!("Creating “{}”", info.name), total, cx) {
            return;
        }
        self.creator = None;
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut step = progress(&this);
            let created = Box::pin(create_from_preset(
                &store, preset, info, namespace, cx, &mut step,
            ))
            .await;
            let outcome = match (created, library, links) {
                (Ok(slot), Some(directory), _) => Box::pin(save_inactive(
                    &store, slot, directory, maker, built, false, cx, &mut step,
                ))
                .await
                .map(|_| SharedString::from(CREATED)),
                (Ok(slot), None, Some(links)) => {
                    Box::pin(relink(&store, slot, links, elsewhere, cx, &mut step))
                        .await
                        .map(|()| SharedString::from(CREATED))
                }
                (Ok(_), None, None) => Ok(SharedString::from(CREATED)),
                (Err(problem), ..) => Err(problem),
            };
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn heading(&self, cx: &Context<Self>) -> Heading {
        let busy = self.busy();
        panel::heading(Page::Profiles.title())
            .standalone()
            .trailing(
                h_flex()
                    .gap(px(ACTION_GAP))
                    .child(
                        ActionButton::new("import-profile", "Import Profile")
                            .icon(Icon::new(Catalog::Download))
                            .disabled(busy || self.library.is_err())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.choose_import(window, cx)),
                            ),
                    )
                    .child(
                        ActionButton::new("new-profile", "New Profile")
                            .icon(Icon::new(Catalog::Plus))
                            .disabled(busy)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_creator(window, cx)),
                            ),
                    ),
            )
    }

    fn onboard_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let busy = self.busy();
        let count = keyboard.profiles.len();
        let capacity = usize::from(keyboard.capabilities.profiles);
        let cards = keyboard
            .profiles
            .iter()
            .enumerate()
            .filter_map(|(index, info)| {
                let slot = Slot::new(u8::try_from(index).ok()?);
                let origin = Origin::Onboard(slot);
                let card = Card {
                    origin,
                    face: Face {
                        name: info.name.as_str().to_owned().into(),
                        color: info.color,
                        icon: info.icon,
                        number: Some(slot.get()),
                        shortcut: keyboard
                            .shortcut(keys::Action::Function(keys::Function::Profile(slot)))
                            .into_iter()
                            .map(|action| SharedString::from(usage::legend(action)))
                            .collect(),
                        apps: None,
                    },
                    active: keyboard.active == slot,
                    default: Self::is_default(slot),
                    menu: self.onboard_entries(slot, count, can_link(keyboard, cx)),
                    status: self.status(origin, cx),
                };
                Some(card.render(cx).into_any_element())
            })
            .collect();
        let tooltip = SharedString::from(format!(
            "Your device can have a total of {capacity} profiles stored on board."
        ));
        card::section(cx)
            .child(
                v_flex()
                    .gap(px(INTRO_GAP))
                    .child(card::section_header(
                        "Onboard profiles",
                        ONBOARD_HELP,
                        Some((count, capacity, tooltip)),
                        cx,
                    ))
                    .child(panel::description(
                        "Drag and drop profiles here to load or swap them.",
                        cx,
                    )),
            )
            .child(card::grid(
                "onboard-profiles",
                cards,
                (count < capacity && !busy).then(|| {
                    card::drop_zone("onboard-drop", Place::NewOnboard, cx).into_any_element()
                }),
            ))
    }

    fn linked_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let busy = self.busy();
        let count = keyboard.linked.len();
        let capacity = usize::from(keyboard.capabilities.linked_profiles);
        let running = service_running(cx);
        let cards: Vec<AnyElement> = (0_u8..)
            .zip(&keyboard.linked)
            .map(|(index, linked)| {
                let slot = Slot::linked(index);
                let origin = Origin::Linked(slot);
                Card {
                    origin,
                    face: Face {
                        name: linked.info.name.as_str().to_owned().into(),
                        color: linked.info.color,
                        icon: linked.info.icon,
                        number: None,
                        shortcut: Vec::new(),
                        apps: apps::summary(&linked.links),
                    },
                    active: keyboard.active == slot,
                    default: false,
                    menu: self.linked_entries(),
                    status: self.status(origin, cx),
                }
                .render(cx)
                .into_any_element()
            })
            .collect();
        let zone = (running && count < capacity && !busy)
            .then(|| card::drop_zone("linked-drop", Place::NewLinked, cx).into_any_element());
        let tooltip = SharedString::from(format!(
            "Your device can have a total of {capacity} linked profiles stored on board."
        ));
        let filled = !cards.is_empty() || zone.is_some();
        card::section(cx)
            .child(
                v_flex()
                    .gap(px(INTRO_GAP))
                    .child(card::section_header(
                        "Linked with an application",
                        LINKED_HELP,
                        Some((count, capacity, tooltip)),
                        cx,
                    ))
                    .child(panel::description(LINKED_SUBHEADER, cx)),
            )
            .children((!running).then(|| notice::service_notice(count, cx)))
            .when(filled, |this| {
                this.child(card::grid("linked-profiles", cards, zone))
            })
    }

    fn linked_entries(&self) -> Vec<(Item, Command)> {
        let library = self.library.is_ok();
        vec![
            (
                Item::new("Edit Profile").icon(Catalog::Pencil),
                Command::Edit,
            ),
            (
                Item::new("Duplicate")
                    .icon(Catalog::Copy)
                    .disabled(!library),
                Command::Duplicate,
            ),
            (
                Item::new("Unlink Profile")
                    .icon(Catalog::Unlink)
                    .disabled(!library),
                Command::Unlink,
            ),
            (
                Item::new("Export...").icon(Catalog::Upload),
                Command::Export,
            ),
            (
                Item::new("Delete...").icon(Catalog::Trash).danger(true),
                Command::Delete,
            ),
        ]
    }

    fn inactive_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let busy = self.busy();
        let room = keyboard.profiles.len() < usize::from(keyboard.capabilities.profiles);
        let header = card::section_header("Inactive profiles", INACTIVE_HELP, None, cx);
        let body = if let Err(problem) = &self.library {
            panel::note(problem.clone(), cx).into_any_element()
        } else {
            let cells: Vec<AnyElement> = self
                .inactive
                .iter()
                .enumerate()
                .map(|(index, stored)| {
                    let origin = Origin::Inactive(index);
                    Card {
                        origin,
                        face: Face {
                            name: stored.file.info.name.as_str().to_owned().into(),
                            color: stored.file.info.color,
                            icon: stored.file.info.icon,
                            number: None,
                            shortcut: Vec::new(),
                            apps: None,
                        },
                        active: false,
                        default: false,
                        menu: inactive_menu(room, can_link(keyboard, cx)),
                        status: self.status(origin, cx),
                    }
                    .render(cx)
                    .into_any_element()
                })
                .collect();
            let zone = (cells.is_empty() && !busy)
                .then(|| card::drop_zone("inactive-drop", Place::Inactive, cx).into_any_element());
            card::grid("inactive-profiles", cells, zone).into_any_element()
        };
        card::section(cx).child(header).child(body)
    }

    pub(crate) fn profile_options(&self, slot: Slot, cx: &Context<Self>) -> Dropdown {
        let keyboard = self.store.read(cx).keyboard();
        let (origin, entries) = if slot.is_linked() {
            (Origin::Linked(slot), self.linked_entries())
        } else {
            let count = keyboard.map_or(0, |keyboard| keyboard.profiles.len());
            let linkable = keyboard.is_some_and(|keyboard| can_link(keyboard, cx));
            (
                Origin::Onboard(slot),
                self.onboard_entries(slot, count, linkable),
            )
        };
        card::options(origin, entries, self.busy(), cx.entity().downgrade())
    }

    pub(crate) const fn is_default(slot: Slot) -> bool {
        slot.get() == 0
    }

    fn onboard_entries(&self, slot: Slot, count: usize, linkable: bool) -> Vec<(Item, Command)> {
        let library = self.library.is_ok();
        let mut menu = vec![
            (
                Item::new("Edit Profile").icon(Catalog::Pencil),
                Command::Edit,
            ),
            (
                Item::new("Duplicate")
                    .icon(Catalog::Copy)
                    .disabled(!library),
                Command::Duplicate,
            ),
            (
                Item::new("Move to Inactive")
                    .icon(Catalog::Archive)
                    .disabled(!library || count <= 1),
                Command::Move,
            ),
        ];
        if linkable {
            menu.push((
                Item::new("Link to App")
                    .icon(Catalog::Link)
                    .disabled(count <= 1 || slot.get() == 0),
                Command::LinkToApp,
            ));
        }
        if slot.get() > 0 {
            menu.push((
                Item::new("Set as Default").icon(Catalog::Star),
                Command::SetDefault,
            ));
        }
        menu.push((
            Item::new("Export...").icon(Catalog::Upload),
            Command::Export,
        ));
        if slot.get() > 0 {
            menu.push((
                Item::new("Delete...").icon(Catalog::Trash).danger(true),
                Command::Delete,
            ));
        }
        menu
    }
}

impl Render for ProfilesPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return panel::page()
                .child(panel::note("Your keyboard is currently disconnected.", cx));
        };
        if let Some(creator) = &self.creator {
            return panel::page().child(creator.clone());
        }
        let job = self.job.as_ref().map(|job| {
            panel::section(job.title.clone(), None, cx)
                .child(Progress::new("profile-job").value(percentage(job.done, job.total)))
        });
        panel::page_with_heading(
            "profiles-scroll",
            self.heading(cx),
            v_flex()
                .w_full()
                .gap(px(SECTION_GAP))
                .children(job)
                .child(self.onboard_section(&keyboard, cx))
                .when(keyboard.capabilities.linked_profiles > 0, |this| {
                    this.child(self.linked_section(&keyboard, cx))
                })
                .child(self.inactive_section(&keyboard, cx)),
        )
        .on_modifiers_changed(cx.listener(|_, _, _, cx| {
            if cx.has_active_drag() {
                cx.notify();
            }
        }))
        .children(drag_hint(window, cx))
    }
}

struct Incoming {
    stored: Stored,
    snapshot: Snapshot,
    copy: bool,
}

struct Relinking {
    info: profile::Info,
    links: Vec<Link>,
    elsewhere: Vec<(Slot, Vec<Link>)>,
}

enum Destination {
    Onboard,
    Linked(Vec<Link>),
    Inactive,
}

fn inactive_menu(room: bool, linkable: bool) -> Vec<(Item, Command)> {
    let mut menu = vec![
        (
            Item::new("Edit Profile").icon(Catalog::Pencil),
            Command::Edit,
        ),
        (
            Item::new("Duplicate").icon(Catalog::Copy),
            Command::Duplicate,
        ),
        (
            Item::new("Move to Onboard")
                .icon(Catalog::Keyboard)
                .disabled(!room),
            Command::Move,
        ),
        (
            Item::new("Copy to Onboard")
                .icon(Catalog::CopyPlus)
                .disabled(!room),
            Command::Copy,
        ),
    ];
    if linkable {
        menu.push((
            Item::new("Link to App").icon(Catalog::Link),
            Command::LinkToApp,
        ));
    }
    menu.extend([
        (
            Item::new("Set as Default").icon(Catalog::Star),
            Command::SetDefault,
        ),
        (
            Item::new("Export...").icon(Catalog::Upload),
            Command::Export,
        ),
        (
            Item::new("Delete...").icon(Catalog::Trash).danger(true),
            Command::Delete,
        ),
    ]);
    menu
}

fn service_running(cx: &App) -> bool {
    Background::global(cx).is_some_and(|background| background.read(cx).is_running())
}

fn can_link(keyboard: &Keyboard, cx: &App) -> bool {
    service_running(cx) && ProfilesPage::linking_room(keyboard)
}

fn drag_hint(window: &Window, cx: &App) -> Option<impl IntoElement> {
    if !cx.has_active_drag() {
        return None;
    }
    let theme = cx.theme();
    let viewport = window.viewport_size();
    Some(
        deferred(
            anchored()
                .anchor(Anchor::BottomLeft)
                .position(point(
                    viewport.width / 2.0,
                    viewport.height - px(HINT_BOTTOM),
                ))
                .child(
                    div()
                        .px(px(HINT_PADDING_X))
                        .py(px(HINT_PADDING_Y))
                        .rounded(px(HINT_RADIUS))
                        .bg(theme.tab_active)
                        .shadow_lg()
                        .text_size(px(HINT_TEXT))
                        .line_height(px(HINT_LINE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(if copy_held(window.modifiers()) {
                            RELEASE_HINT
                        } else {
                            COPY_HINT
                        }),
                ),
        )
        .with_priority(2),
    )
}

const fn copy_held(modifiers: Modifiers) -> bool {
    modifiers.control
}

fn progress(this: &WeakEntity<ProfilesPage>) -> impl FnMut(&mut AsyncApp) + '_ {
    move |cx| {
        this.update(cx, ProfilesPage::advance).ok();
    }
}

fn onboard_of(keyboard: &Keyboard) -> Onboard {
    Onboard {
        active: keyboard.active,
        count: u8::try_from(keyboard.profiles.len()).unwrap_or(u8::MAX),
        lightbar: keyboard.capabilities.lightbar_leds > 0,
    }
}

fn snapshot_of(stored: &Stored, keyboard: &Keyboard) -> Result<Snapshot, String> {
    let mut parts = stored.file.parts.clone();
    if keyboard.capabilities.lightbar_leds == 0 {
        parts.lightbar = None;
    }
    parts.check(&keyboard.capabilities).map_err(|mismatch| {
        format!(
            "“{}” does not fit this keyboard: {mismatch}",
            stored.file.info.name
        )
    })?;
    Ok(Snapshot {
        info: stored.file.info.clone(),
        parts,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the operation needs the keyboard, the library and the progress callback"
)]
async fn save_inactive(
    store: &Entity<Store>,
    slot: Slot,
    directory: PathBuf,
    maker: String,
    onboard: Onboard,
    copy: bool,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let snapshot = transfer::read(store, slot, onboard.lightbar, cx, step).await?;
    let file = ProfileFile::new(maker, snapshot.info, snapshot.parts);
    library::on_worker(move || library::add(&directory, file))
        .await
        .map_err(|error| error_chain(&error))?;
    step(cx);
    if copy {
        return Ok("The profile has been duplicated.".into());
    }
    transfer::remove(store, slot, onboard, cx, step).await?;
    Ok("The profile has been moved to inactive profiles.".into())
}

async fn load_inactive(
    store: &Entity<Store>,
    stored: Stored,
    snapshot: Snapshot,
    copy: bool,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let slot = transfer::insert(store, &snapshot, Namespace::Onboard, cx, step).await?;
    if !copy {
        library::on_worker(move || library::remove(&stored))
            .await
            .map_err(|error| error_chain(&error))?;
    }
    step(cx);
    Ok(format!(
        "The profile has been loaded into slot {}.",
        u16::from(slot.get()) + 1
    )
    .into())
}

#[expect(
    clippy::too_many_arguments,
    reason = "the operation needs the keyboard, the library and the progress callback"
)]
async fn exchange(
    store: &Entity<Store>,
    incoming: Incoming,
    slot: Slot,
    directory: PathBuf,
    maker: String,
    onboard: Onboard,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let outgoing = transfer::read(store, slot, onboard.lightbar, cx, step).await?;
    let file = ProfileFile::new(maker, outgoing.info, outgoing.parts);
    library::on_worker(move || library::add(&directory, file))
        .await
        .map_err(|error| error_chain(&error))?;
    step(cx);
    transfer::replace(store, slot, &incoming.snapshot, onboard, cx, step).await?;
    if !incoming.copy {
        let stored = incoming.stored;
        library::on_worker(move || library::remove(&stored))
            .await
            .map_err(|error| error_chain(&error))?;
    }
    step(cx);
    Ok(format!(
        "The profile has been loaded into slot {}.",
        u16::from(slot.get()) + 1
    )
    .into())
}

async fn export(
    store: &Entity<Store>,
    slot: Slot,
    maker: String,
    path: PathBuf,
    onboard: Onboard,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let snapshot = transfer::read(store, slot, onboard.lightbar, cx, step).await?;
    let file = ProfileFile::new(maker, snapshot.info, snapshot.parts);
    let target = path.clone();
    library::on_worker(move || file.save(&target))
        .await
        .map_err(|error| error_chain(&error))?;
    step(cx);
    Ok(format!("The profile has been exported to {}.", path.display()).into())
}

async fn import(directory: PathBuf, path: PathBuf, keyboard: Keyboard) -> Outcome {
    let source = path.clone();
    let mut file = library::on_worker(move || ProfileFile::load(&source))
        .await
        .map_err(|error| error_chain(&error))?;
    if keyboard.capabilities.lightbar_leds == 0 {
        file.parts.lightbar = None;
    }
    file.parts
        .check(&keyboard.capabilities)
        .map_err(|mismatch| {
            format!(
                "`{}` does not fit this keyboard: {mismatch}",
                path.display()
            )
        })?;
    let made_elsewhere = file.keyboard != keyboard.name.as_ref();
    library::on_worker(move || library::add(&directory, file))
        .await
        .map_err(|error| error_chain(&error))?;
    Ok(if made_elsewhere {
        "While most features will work, some settings might be off due to different layouts."
    } else {
        "The profile has been imported."
    }
    .into())
}

async fn link_onboard_profile(
    store: &Entity<Store>,
    slot: Slot,
    onboard: Onboard,
    linking: Relinking,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let mut snapshot = transfer::read(store, slot, onboard.lightbar, cx, step).await?;
    snapshot.info = linking.info;
    let linked = transfer::insert(store, &snapshot, Namespace::Linked, cx, step).await?;
    relink(store, linked, linking.links, linking.elsewhere, cx, step).await?;
    transfer::remove(store, slot, onboard, cx, step).await?;
    Ok(LINKED.into())
}

async fn link_inactive_profile(
    store: &Entity<Store>,
    stored: Stored,
    snapshot: Snapshot,
    links: Vec<Link>,
    elsewhere: Vec<(Slot, Vec<Link>)>,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let linked = transfer::insert(store, &snapshot, Namespace::Linked, cx, step).await?;
    relink(store, linked, links, elsewhere, cx, step).await?;
    library::on_worker(move || library::remove(&stored))
        .await
        .map_err(|error| error_chain(&error))?;
    step(cx);
    Ok(LINKED.into())
}

async fn relink(
    store: &Entity<Store>,
    linked: Slot,
    links: Vec<Link>,
    elsewhere: Vec<(Slot, Vec<Link>)>,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<(), String> {
    for (other, kept) in elsewhere {
        store
            .update(cx, |store, cx| store.write_app_links(other, kept, cx))
            .await
            .map_err(|error| error_chain(&error))?;
    }
    transfer::link_apps(store, linked, links, cx, step).await
}

#[expect(
    clippy::too_many_arguments,
    reason = "the operation needs the keyboard, the library and the progress callback"
)]
async fn unlink_profile(
    store: &Entity<Store>,
    slot: Slot,
    directory: PathBuf,
    maker: String,
    links: Vec<Link>,
    onboard: Onboard,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let snapshot = transfer::read(store, slot, onboard.lightbar, cx, step).await?;
    let mut file = ProfileFile::new(maker, snapshot.info, snapshot.parts);
    file.links = links;
    library::on_worker(move || library::add(&directory, file))
        .await
        .map_err(|error| error_chain(&error))?;
    step(cx);
    Box::pin(transfer::remove_linked(
        store,
        slot,
        onboard.lightbar,
        cx,
        step,
    ))
    .await?;
    Ok("The profile has been moved to inactive profiles.".into())
}

async fn onboard_linked_profile(
    store: &Entity<Store>,
    slot: Slot,
    onboard: Onboard,
    copy: bool,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let snapshot = transfer::read(store, slot, onboard.lightbar, cx, step).await?;
    let loaded = transfer::insert(store, &snapshot, Namespace::Onboard, cx, step).await?;
    if !copy {
        Box::pin(transfer::remove_linked(
            store,
            slot,
            onboard.lightbar,
            cx,
            step,
        ))
        .await?;
    }
    Ok(format!(
        "The profile has been loaded into slot {}.",
        u16::from(loaded.get()) + 1
    )
    .into())
}

fn links_elsewhere(
    keyboard: &Keyboard,
    except: Option<Slot>,
    links: &[Link],
) -> Vec<(Slot, Vec<Link>)> {
    let taken: Vec<String> = links.iter().map(app_watcher::identity).collect();
    (0_u8..)
        .zip(&keyboard.linked)
        .map(|(index, linked)| (Slot::linked(index), linked))
        .filter(|(slot, _)| Some(*slot) != except)
        .filter_map(|(slot, linked)| {
            let kept: Vec<Link> = linked
                .links
                .iter()
                .filter(|link| !taken.contains(&app_watcher::identity(link)))
                .cloned()
                .collect();
            (kept.len() != linked.links.len()).then_some((slot, kept))
        })
        .collect()
}

fn chosen_path<E, F>(chosen: Result<Result<Option<PathBuf>, E>, F>) -> Option<PathBuf> {
    match chosen {
        Ok(Ok(Some(path))) => Some(path),
        _ => None,
    }
}

const fn preset_steps(preset: Preset) -> usize {
    let mut steps = 3;
    if preset.changes_keys() || preset.changes_gamepad() {
        steps += 1;
    }
    if preset.changes_keys() {
        steps += 4;
    }
    if preset.changes_gamepad() {
        steps += 2;
    }
    if preset.changes_lighting() {
        steps += 2;
    }
    steps
}

async fn create_from_preset(
    store: &Entity<Store>,
    preset: Preset,
    info: profile::Info,
    namespace: Namespace,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<Slot, String> {
    store.update(cx, Store::clear_history);
    let slot = store
        .update(cx, |store, cx| store.create_profile(namespace, cx))
        .await
        .ok_or("the keyboard did not create a profile")?;
    step(cx);
    let tuned = tune_preset(store, slot, preset, cx, step).await;
    let change = Change {
        profile: slot,
        value: info,
    };
    apply::<SetProfileInfo>(store, change, Scope::Profile(slot), cx)
        .await
        .map_err(|error| error_chain(&error))?;
    step(cx);
    store.update(cx, |store, cx| store.reload(cx)).await;
    step(cx);
    tuned.map_err(|problem| {
        format!(
            "profile {} was created, but the template could not be applied: {problem}",
            u16::from(slot.index()) + 1
        )
    })?;
    Ok(slot)
}

async fn tune_preset(
    store: &Entity<Store>,
    slot: Slot,
    preset: Preset,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<(), String> {
    let describe = |error: crate::device::Failure| error_chain(&error);
    let scope = Scope::Profile(slot);
    if preset.changes_keys() || preset.changes_gamepad() {
        let base = LayerRef {
            profile: slot,
            layer: Layer::Base,
        };
        let keymap = fetch::<GetKeymap, Keymap>(store, base, cx)
            .await
            .map_err(describe)?;
        step(cx);
        let movement = Movement::find(&keymap);
        if preset.changes_keys() {
            let keyboard = store
                .read_with(cx, |store, _| store.keyboard().cloned())
                .ok_or("the keyboard was disconnected")?;
            let mut settings = fetch::<GetKeySettings, keys::Settings>(store, slot, cx)
                .await
                .map_err(describe)?;
            step(cx);
            let mut overrides = fetch::<GetKeyOverrides, keys::Overrides>(store, slot, cx)
                .await
                .map_err(describe)?;
            step(cx);
            preset.tune_keys(
                &mut settings,
                &mut overrides,
                movement,
                |key, millimetres| keyboard.distance_from_mm(key, millimetres),
            );
            let settings = Change {
                profile: slot,
                value: settings,
            };
            apply::<SetKeySettings>(store, settings, scope, cx)
                .await
                .map_err(describe)?;
            step(cx);
            let overrides = Change {
                profile: slot,
                value: overrides,
            };
            apply::<SetKeyOverrides>(store, overrides, scope, cx)
                .await
                .map_err(describe)?;
            step(cx);
        }
        if preset.changes_gamepad() {
            let mut mapping = fetch::<GetGamepadMapping, gamepad::Mapping>(store, slot, cx)
                .await
                .map_err(describe)?;
            step(cx);
            preset.map_gamepad(&mut mapping, movement);
            let mapping = Change {
                profile: slot,
                value: mapping,
            };
            apply::<SetGamepadMapping>(store, mapping, scope, cx)
                .await
                .map_err(describe)?;
            step(cx);
        }
    }
    if preset.changes_lighting() {
        let mut settings = fetch::<GetLighting, lighting::Settings>(store, slot, cx)
            .await
            .map_err(describe)?;
        step(cx);
        preset.tune_lighting(&mut settings);
        let settings = Change {
            profile: slot,
            value: settings,
        };
        apply::<SetLighting>(store, settings, scope, cx)
            .await
            .map_err(describe)?;
        step(cx);
    }
    Ok(())
}

fn profile_name(text: &str) -> Result<Name, String> {
    if text.trim().is_empty() {
        return Err("Enter a name.".to_owned());
    }
    Name::try_from(text).map_err(|_| format!("Up to {NAME_LEN} bytes."))
}

fn suggested_file(name: &str) -> String {
    format!("{}.{EXTENSION}", file_stem(name))
}

fn file_stem(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        "profile".to_owned()
    } else {
        stem
    }
}

fn home_directory() -> PathBuf {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map_or_else(|| Path::new(".").to_owned(), PathBuf::from)
}

fn percentage(done: usize, total: usize) -> f32 {
    let done = u32::try_from(done).unwrap_or(u32::MAX);
    let total = u32::try_from(total).unwrap_or(u32::MAX).max(1);
    #[expect(
        clippy::cast_precision_loss,
        reason = "step counts stay far below the 24-bit mantissa"
    )]
    let ratio = done as f32 / total as f32;
    (ratio * 100.0).clamp(0.0, 100.0)
}

fn error_chain(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

fn sentence(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_longer_than_the_limit_are_refused() {
        assert!(profile_name("Rapid").is_ok());
        assert!(profile_name("   ").is_err());
        assert!(profile_name(&"x".repeat(NAME_LEN)).is_ok());
        assert!(profile_name(&"x".repeat(NAME_LEN + 1)).is_err());
        assert!(profile_name(&"é".repeat(17)).is_err());
    }

    #[test]
    fn file_stems_drop_path_characters() {
        assert_eq!(file_stem("FPS / Valorant"), "FPS _ Valorant");
        assert_eq!(file_stem("  "), "profile");
        assert_eq!(suggested_file("Rapid"), "Rapid.json");
    }

    #[test]
    fn steps_cover_every_request_of_a_template() {
        assert_eq!(preset_steps(Preset::Custom), 3);
        assert_eq!(preset_steps(Preset::Fps), 8);
        assert_eq!(preset_steps(Preset::Racing), 6);
        assert_eq!(preset_steps(Preset::Typing), 10);
    }

    #[test]
    fn percentage_of_finished_steps() {
        assert!((percentage(1, 4) - 25.0).abs() < f32::EPSILON);
        assert!((percentage(9, 4) - 100.0).abs() < f32::EPSILON);
    }
}
