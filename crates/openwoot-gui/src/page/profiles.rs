mod card;
mod creator;
mod editor;
mod file;
mod icon;
mod library;
mod preset;
mod shortcuts;
mod transfer;

use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
};

use gpui_kit::{
    AnyElement, App, AppContext as _, AsyncApp, Context, Div, Entity, FontWeight, IntoElement,
    Modifiers, ParentElement as _, PathPromptOptions, Render, SharedString, Styled as _,
    Subscription, WeakEntity, Window, anchored,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariant},
        h_flex,
        notification::Notification,
        progress::Progress,
        v_flex,
    },
    deferred, div, point, px,
};
use openwoot_protocol::{
    endpoints::{
        GetGamepadMapping, GetKeyOverrides, GetKeySettings, GetKeymap, GetLighting,
        SetGamepadMapping, SetKeyOverrides, SetKeySettings, SetLighting, SetProfileInfo,
    },
    gamepad, heapless,
    keys::{self, Keymap, Layer},
    lighting,
    profile::{self, Change, LayerRef, NAME_LEN, Slot},
};

pub(crate) use self::icon::avatar;
use self::{
    card::{Card, Moving, Origin, Place},
    creator::Creator,
    editor::Editor,
    file::{EXTENSION, ProfileFile},
    library::Stored,
    preset::{Movement, Preset},
    transfer::{Onboard, Snapshot},
};
use super::Page;
use crate::{
    dropdown::Item,
    panel,
    profile_parts::{apply, fetch},
    store::{Keyboard, Scope, Store, StoreEvent},
};

type Name = heapless::String<NAME_LEN>;
type Outcome = Result<SharedString, String>;

const ONBOARD_HELP: &str = "Onboard profiles are stored on the keyboard and work on any computer. A card shows the shortcut that switches to it; change it on the Remap page.";
const INACTIVE_HELP: &str = "Inactive profiles are stored on this computer only. Drag one onto the onboard profiles to load it onto the keyboard.";
const HINT_WIDTH: f32 = 300.0;
const HINT_BOTTOM: f32 = 72.0;
const COPY_HINT: &str = if cfg!(target_os = "macos") {
    "Hold Option to copy instead of move"
} else {
    "Hold Ctrl to copy instead of move"
};

pub(crate) struct ProfilesPage {
    store: Entity<Store>,
    library: Result<PathBuf, SharedString>,
    inactive: Vec<Stored>,
    keymaps: Vec<(Layer, Keymap)>,
    job: Option<Job>,
    creator: Option<Entity<Creator>>,
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
    Inactive(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Edit,
    Duplicate,
    Move,
    Copy,
    SetDefault,
    Export,
    Delete,
}

impl ProfilesPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, _, cx| match event {
                StoreEvent::Connected | StoreEvent::ProfileChanged | StoreEvent::Reloaded => {
                    this.refresh_keymaps(cx);
                }
                StoreEvent::Disconnected => {
                    this.keymaps.clear();
                    this.creator = None;
                    cx.notify();
                }
                StoreEvent::Failed(_) => {}
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
        ];
        let library = library::directory().map_err(|error| SharedString::from(error_chain(&error)));
        let page = Self {
            store,
            library,
            inactive: Vec::new(),
            keymaps: Vec::new(),
            job: None,
            creator: None,
            _subscriptions: subscriptions,
        };
        page.refresh_keymaps(cx);
        page.load_library(cx);
        page
    }

    fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn refresh_keymaps(&self, cx: &mut Context<Self>) {
        let Some(active) = self.store.read(cx).active() else {
            return;
        };
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let mut keymaps = Vec::new();
            for layer in Layer::ALL.into_iter().filter(|layer| *layer != Layer::Base) {
                let request = LayerRef {
                    profile: active,
                    layer,
                };
                if let Ok(keymap) = fetch::<GetKeymap, Keymap>(&store, request, cx).await {
                    keymaps.push((layer, keymap));
                }
            }
            this.update(cx, |page, cx| {
                page.keymaps = keymaps;
                cx.notify();
            })
            .ok();
        })
        .detach();
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
            Err(problem) => window.push_notification(Notification::error(problem), cx),
        }
        self.load_library(cx);
        cx.notify();
    }

    fn activate(&self, slot: Slot, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.activate(slot, cx))
            .detach();
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
            (Origin::Inactive(index), Command::Edit) => self.edit_inactive(index, window, cx),
            (Origin::Onboard(slot), Command::Duplicate) => {
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
            (Origin::Onboard(slot), Command::SetDefault) => {
                self.swap(slot, Slot::new(0), window, cx);
            }
            (Origin::Inactive(index), Command::SetDefault) => {
                self.exchange(index, Slot::new(0), false, window, cx);
            }
            (Origin::Onboard(slot), Command::Export) => self.export_onboard(slot, window, cx),
            (Origin::Inactive(index), Command::Export) => self.export_inactive(index, window, cx),
            (Origin::Onboard(slot), Command::Delete) => {
                self.confirm_delete(Origin::Onboard(slot), window, cx);
            }
            (Origin::Inactive(index), Command::Delete) => {
                self.confirm_delete(Origin::Inactive(index), window, cx);
            }
            (Origin::Onboard(_), Command::Copy) => {}
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
            _ => {}
        }
    }

    fn edit_onboard(&self, slot: Slot, window: &mut Window, cx: &mut Context<Self>) {
        let Some(info) = self
            .store
            .read(cx)
            .keyboard()
            .and_then(|keyboard| keyboard.profiles.get(usize::from(slot.get())))
            .cloned()
        else {
            return;
        };
        Editor::open(
            cx.entity().downgrade(),
            Subject::Onboard(slot),
            &info,
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
        Editor::open(cx.entity().downgrade(), subject, &info, window, cx);
    }

    fn apply_edit(
        &mut self,
        subject: Subject,
        info: profile::Info,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match subject {
            Subject::Onboard(slot) => {
                self.store
                    .update(cx, |store, cx| store.set_profile_info(slot, info, cx))
                    .detach();
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
        }
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
                Notification::warning("The keyboard has no free profile slot"),
                cx,
            );
            return;
        }
        let snapshot = match snapshot_of(&stored, keyboard) {
            Ok(snapshot) => snapshot,
            Err(problem) => {
                window.push_notification(Notification::error(problem), cx);
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
                window.push_notification(Notification::error(problem), cx);
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
            Origin::Onboard(slot) => self
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
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            alert
                .title("Delete profile")
                .description(format!("“{name}” is removed for good."))
                .confirm()
                .ok_text("Delete profile")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    page.update(cx, |page, cx| page.delete(origin, window, cx))
                        .ok();
                    true
                })
        });
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
                        .map(|()| SharedString::from("Profile deleted"));
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
                        .map(|()| SharedString::from("Profile deleted"))
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
                .map(|()| SharedString::from(format!("Exported to {}", path.display())))
                .map_err(|error| error_chain(&error));
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn choose_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(directory) = self.library.clone() else {
            return;
        };
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import profile".into()),
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
                None => Err("connect a keyboard to import a profile".to_owned()),
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
        inactive: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let onboard = onboard_of(keyboard);
        let maker = keyboard.name.to_string();
        let library = if inactive {
            match self.library.clone() {
                Ok(directory) => Some(directory),
                Err(_) => return,
            }
        } else {
            None
        };
        let built = Onboard {
            count: onboard.count.saturating_add(1),
            ..onboard
        };
        let total = preset_steps(preset)
            + library
                .as_ref()
                .map_or(0, |_| built.copy_steps() + 1 + transfer::REMOVE_STEPS);
        if !self.begin(format!("Creating “{}”", info.name), total, cx) {
            return;
        }
        self.creator = None;
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut step = progress(&this);
            let created = Box::pin(create_from_preset(&store, preset, info, cx, &mut step)).await;
            let outcome = match (created, library) {
                (Ok(slot), Some(directory)) => Box::pin(save_inactive(
                    &store, slot, directory, maker, built, false, cx, &mut step,
                ))
                .await
                .map(|_| SharedString::from("Created in inactive profiles")),
                (Ok(slot), None) => {
                    Ok(format!("Created profile {}", u16::from(slot.get()) + 1).into())
                }
                (Err(problem), _) => Err(problem),
            };
            this.update_in(cx, |page, window, cx| page.finish(outcome, window, cx))
                .ok();
        })
        .detach();
    }

    fn heading(&self, cx: &Context<Self>) -> Div {
        let busy = self.busy();
        panel::heading(
            Page::Profiles.title(),
            None,
            h_flex()
                .gap_2()
                .child(
                    Button::new("import-profile")
                        .small()
                        .icon(Catalog::Download)
                        .label("Import profile")
                        .disabled(busy || self.library.is_err())
                        .on_click(
                            cx.listener(|this, _, window, cx| this.choose_import(window, cx)),
                        ),
                )
                .child(
                    Button::new("new-profile")
                        .small()
                        .icon(Catalog::Plus)
                        .label("New profile")
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, window, cx| this.open_creator(window, cx))),
                ),
            cx,
        )
    }

    fn onboard_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let busy = self.busy();
        let count = keyboard.profiles.len();
        let capacity = usize::from(keyboard.capabilities.profiles);
        let mut keymaps = vec![(Layer::Base, keyboard.legends.clone())];
        keymaps.extend(self.keymaps.iter().cloned());
        let cards = keyboard
            .profiles
            .iter()
            .enumerate()
            .filter_map(|(index, info)| {
                let slot = Slot::new(u8::try_from(index).ok()?);
                let card = Card {
                    origin: Origin::Onboard(slot),
                    name: info.name.as_str().to_owned().into(),
                    color: info.color,
                    icon: info.icon,
                    number: Some(slot.get()),
                    active: keyboard.active == slot,
                    default: index == 0,
                    shortcut: shortcuts::shortcut(&keymaps, slot),
                    menu: self.onboard_menu(slot, count),
                    busy,
                };
                Some(card.render(cx).into_any_element())
            })
            .collect();
        let tooltip = SharedString::from(format!("The keyboard stores up to {capacity} profiles."));
        card::section(cx)
            .child(
                v_flex()
                    .gap_2()
                    .child(card::section_header(
                        "Onboard profiles",
                        ONBOARD_HELP,
                        Some((format!("{count} / {capacity}").into(), tooltip)),
                        cx,
                    ))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Drop a profile here to load it onto the keyboard; drop it on another to swap them."),
                    ),
            )
            .child(card::grid(cards))
            .children(
                (count < capacity && !busy)
                    .then(|| card::drop_zone("onboard-drop", Place::NewOnboard, cx)),
            )
    }

    fn inactive_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let busy = self.busy();
        let room = keyboard.profiles.len() < usize::from(keyboard.capabilities.profiles);
        let header = card::section_header("Inactive profiles", INACTIVE_HELP, None, cx);
        let body = if let Err(problem) = &self.library {
            div()
                .text_sm()
                .text_color(theme.danger)
                .child(problem.clone())
        } else {
            let mut cells: Vec<AnyElement> = self
                .inactive
                .iter()
                .enumerate()
                .map(|(index, stored)| {
                    Card {
                        origin: Origin::Inactive(index),
                        name: stored.file.info.name.as_str().to_owned().into(),
                        color: stored.file.info.color,
                        icon: stored.file.info.icon,
                        number: None,
                        active: false,
                        default: false,
                        shortcut: Vec::new(),
                        menu: inactive_menu(room),
                        busy,
                    }
                    .render(cx)
                    .into_any_element()
                })
                .collect();
            if !busy {
                cells
                    .push(card::drop_zone("inactive-drop", Place::Inactive, cx).into_any_element());
            }
            card::grid(cells)
        };
        card::section(cx).child(header).child(body)
    }

    fn onboard_menu(&self, slot: Slot, count: usize) -> Vec<(Item, Command)> {
        let library = self.library.is_ok();
        let mut menu = vec![
            (
                Item::new("Edit profile").icon(Catalog::Pencil),
                Command::Edit,
            ),
            (
                Item::new("Duplicate")
                    .icon(Catalog::Copy)
                    .disabled(!library),
                Command::Duplicate,
            ),
            (
                Item::new("Move to inactive")
                    .icon(Catalog::Archive)
                    .disabled(!library || count <= 1),
                Command::Move,
            ),
        ];
        if slot.get() > 0 {
            menu.push((
                Item::new("Set as default").icon(Catalog::Star),
                Command::SetDefault,
            ));
        }
        menu.push((Item::new("Export…").icon(Catalog::Upload), Command::Export));
        if slot.get() > 0 {
            menu.push((Item::new("Delete…").icon(Catalog::Trash), Command::Delete));
        }
        menu
    }
}

impl Render for ProfilesPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = panel::page();
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return page.child(panel::note("No keyboard", cx));
        };
        if let Some(creator) = &self.creator {
            return page.child(creator.clone());
        }
        let job = self.job.as_ref().map(|job| {
            panel::section(job.title.clone(), None, cx)
                .child(Progress::new("profile-job").value(percentage(job.done, job.total)))
        });
        page.child(self.heading(cx))
            .children(job)
            .child(self.onboard_section(&keyboard, cx))
            .child(self.inactive_section(&keyboard, cx))
            .children(drag_hint(window, cx))
    }
}

struct Incoming {
    stored: Stored,
    snapshot: Snapshot,
    copy: bool,
}

fn inactive_menu(room: bool) -> Vec<(Item, Command)> {
    vec![
        (
            Item::new("Edit profile").icon(Catalog::Pencil),
            Command::Edit,
        ),
        (
            Item::new("Duplicate").icon(Catalog::Copy),
            Command::Duplicate,
        ),
        (
            Item::new("Move to onboard")
                .icon(Catalog::Keyboard)
                .disabled(!room),
            Command::Move,
        ),
        (
            Item::new("Copy to onboard")
                .icon(Catalog::CopyPlus)
                .disabled(!room),
            Command::Copy,
        ),
        (
            Item::new("Set as default").icon(Catalog::Star),
            Command::SetDefault,
        ),
        (Item::new("Export…").icon(Catalog::Upload), Command::Export),
        (Item::new("Delete…").icon(Catalog::Trash), Command::Delete),
    ]
}

fn drag_hint(window: &Window, cx: &App) -> Option<impl IntoElement> {
    if !cx.has_active_drag() {
        return None;
    }
    let theme = cx.theme();
    let viewport = window.viewport_size();
    let origin = point(
        viewport.width / 2.0 - px(HINT_WIDTH / 2.0),
        viewport.height - px(HINT_BOTTOM),
    );
    Some(
        deferred(
            anchored().position(origin).child(
                div()
                    .w(px(HINT_WIDTH))
                    .py_2()
                    .rounded(theme.radius)
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .text_center()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(COPY_HINT),
            ),
        )
        .with_priority(2),
    )
}

fn copy_held(modifiers: Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.alt
    } else {
        modifiers.control
    }
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
        return Ok("Duplicated into inactive profiles".into());
    }
    transfer::remove(store, slot, onboard, cx, step).await?;
    Ok("Moved to inactive profiles".into())
}

async fn load_inactive(
    store: &Entity<Store>,
    stored: Stored,
    snapshot: Snapshot,
    copy: bool,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let slot = transfer::insert(store, &snapshot, cx, step).await?;
    if !copy {
        library::on_worker(move || library::remove(&stored))
            .await
            .map_err(|error| error_chain(&error))?;
    }
    step(cx);
    Ok(format!("Loaded into slot {}", u16::from(slot.get()) + 1).into())
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
    Ok(format!("Loaded into slot {}", u16::from(slot.get()) + 1).into())
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
    Ok(format!("Exported to {}", path.display()).into())
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
    let name = file.info.name.clone();
    library::on_worker(move || library::add(&directory, file))
        .await
        .map_err(|error| error_chain(&error))?;
    Ok(if made_elsewhere {
        format!("Imported “{name}”; it was made for another keyboard, so some keys may differ")
    } else {
        format!("Imported “{name}” into inactive profiles")
    }
    .into())
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
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<Slot, String> {
    store.update(cx, Store::clear_history);
    let slot = store
        .update(cx, |store, cx| store.create_profile(cx))
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
            u16::from(slot.get()) + 1
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
