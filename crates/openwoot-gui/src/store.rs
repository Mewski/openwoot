use std::{rc::Rc, time::Duration};

use async_channel::Receiver;
use gpui_kit::{Context, EventEmitter, SharedString, Task};
use openwoot_protocol::{
    Endpoint, Version, apps,
    configuration::{Capabilities, Rejection},
    device::{self, Layout, LayoutState},
    endpoints::{
        ActivateProfile, CreateProfile, DeleteAppLink, DeleteProfile, GetAppLink, GetAppLinkCount,
        GetCapabilities, GetDeviceName, GetKeyGeometry, GetKeymap, GetLayout, GetProfileInfo,
        GetProfiles, GetSwitchModels, GetSwitches, Revert, Save, SelectLinkedProfile, SetAppLink,
        SetProfileInfo,
    },
    geometry,
    keys::{self, Action, Distance, Layer},
    profile::{self, Change, LayerRef, Namespace, Selection, Slot},
    switches,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    device::{Device, Failure, Presence},
    history::{History, Replay},
};

const PROFILE_POLL_INTERVAL: Duration = Duration::from_secs(1);
const MILLIMETRES_PER_TRAVEL_STEP: f32 = 0.05;
const FALLBACK_TRAVEL_MM: f32 = 4.0;
const FN_LAYERS: [Layer; 3] = [Layer::One, Layer::Two, Layer::Three];

pub(crate) struct Store {
    device: Device,
    link: Link,
    keyboard: Option<Keyboard>,
    searching: bool,
    history: History,
    _presence: Task<()>,
    _poll: Task<()>,
}

#[derive(Debug, Clone)]
pub(crate) enum Link {
    Searching,
    Loading,
    Connected,
    Unreachable(SharedString),
}

#[derive(Debug, Clone)]
pub(crate) struct Keyboard {
    pub(crate) name: SharedString,
    pub(crate) device_name: Option<SharedString>,
    pub(crate) version: Version,
    pub(crate) capabilities: Capabilities,
    pub(crate) geometry: Option<geometry::Keys>,
    pub(crate) layout: Option<Layout>,
    pub(crate) profiles: Vec<profile::Info>,
    pub(crate) linked: Vec<Linked>,
    pub(crate) selected: Option<Slot>,
    pub(crate) active: Slot,
    pub(crate) legends: keys::Keymap,
    fn_keymaps: Vec<(Layer, keys::Keymap)>,
    pub(crate) switches: Option<switches::Settings>,
    pub(crate) models: switches::Models,
    pub(crate) unsaved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Linked {
    pub(crate) info: profile::Info,
    pub(crate) links: Vec<apps::Link>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Profile(Slot),
    Keyboard,
    Runtime,
}

#[derive(Debug, Clone)]
pub(crate) enum StoreEvent {
    Connected,
    Disconnected,
    ProfileChanged,
    Reloaded,
    Failed(SharedString),
}

struct ProfileSet {
    active: Slot,
    profiles: Vec<profile::Info>,
    linked: Vec<Linked>,
    selected: Option<Slot>,
    legends: keys::Keymap,
    fn_keymaps: Vec<(Layer, keys::Keymap)>,
}

impl EventEmitter<StoreEvent> for Store {}

impl Store {
    pub(crate) fn new(
        device: Device,
        presence: Receiver<Presence>,
        cx: &mut Context<Self>,
    ) -> Self {
        let presence_task = cx.spawn(async move |this, cx| {
            while let Ok(update) = presence.recv().await {
                if this
                    .update(cx, |store, cx| store.on_presence(update, cx))
                    .is_err()
                {
                    return;
                }
            }
        });
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PROFILE_POLL_INTERVAL).await;
                let Ok(Some(check)) = this.update(cx, |store, cx| store.poll_profiles(cx)) else {
                    if this.upgrade().is_none() {
                        return;
                    }
                    continue;
                };
                check.await;
            }
        });
        Self {
            device,
            link: Link::Searching,
            keyboard: None,
            searching: false,
            history: History::default(),
            _presence: presence_task,
            _poll: poll,
        }
    }

    pub(crate) const fn link(&self) -> &Link {
        &self.link
    }

    pub(crate) const fn keyboard(&self) -> Option<&Keyboard> {
        self.keyboard.as_ref()
    }

    pub(crate) fn device(&self) -> Device {
        self.device.clone()
    }

    pub(crate) const fn is_searching(&self) -> bool {
        self.searching
    }

    pub(crate) fn search(&mut self, cx: &mut Context<Self>) {
        self.searching = true;
        self.device.search();
        cx.notify();
    }

    pub(crate) fn active(&self) -> Option<Slot> {
        self.keyboard.as_ref().map(|keyboard| keyboard.active)
    }

    pub(crate) fn fetch<E, T>(
        &self,
        request: E::Request,
        cx: &mut Context<Self>,
    ) -> Task<Result<T, Failure>>
    where
        E: Endpoint<Response = Result<T, Rejection>>,
        E::Request: Serialize + Send + 'static,
        T: DeserializeOwned + Send + 'static,
    {
        let call = self.device.configure::<E, T>(request);
        cx.spawn(async move |this, cx| {
            let answer = call.await;
            if let Err(error) = &answer {
                this.update(cx, |_, cx| {
                    cx.emit(StoreEvent::Failed(error.to_string().into()));
                })
                .ok();
            }
            answer
        })
    }

    pub(crate) fn apply<E>(
        &self,
        request: E::Request,
        scope: Scope,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), Failure>>
    where
        E: Endpoint<Response = Result<(), Rejection>>,
        E::Request: Serialize + Send + 'static,
    {
        let call = self.device.configure::<E, ()>(request);
        cx.spawn(async move |this, cx| {
            let answer = call.await;
            this.update(cx, |store, cx| match &answer {
                Ok(()) => store.mark_changed(scope, cx),
                Err(error) => cx.emit(StoreEvent::Failed(error.to_string().into())),
            })
            .ok();
            answer
        })
    }

    pub(crate) fn change<E>(
        &mut self,
        before: &E::Request,
        after: E::Request,
        scope: Scope,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), Failure>>
    where
        E: Endpoint<Response = Result<(), Rejection>>,
        E::Request: Clone + Serialize + Send + 'static,
    {
        self.history.record::<E>(before, &after, scope);
        cx.notify();
        self.apply::<E>(after, scope, cx)
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub(crate) fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(restore) = self.history.undo() {
            self.replay(&restore, cx);
        }
    }

    pub(crate) fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(reapply) = self.history.redo() {
            self.replay(&reapply, cx);
        }
    }

    fn replay(&self, replay: &Replay, cx: &mut Context<Self>) {
        let sent = replay(self, cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            if sent.await.is_ok()
                && let Ok(reload) = this.update(cx, |store, cx| store.reload(cx))
            {
                reload.await;
            }
        })
        .detach();
    }

    pub(crate) fn save(&self, cx: &mut Context<Self>) -> Task<Result<(), Failure>> {
        let call = self.device.configure::<Save, ()>(());
        cx.spawn(async move |this, cx| {
            let answer = call.await;
            this.update(cx, |store, cx| {
                match &answer {
                    Ok(()) => {
                        if let Some(keyboard) = &mut store.keyboard {
                            keyboard.unsaved = false;
                        }
                    }
                    Err(error) => cx.emit(StoreEvent::Failed(error.to_string().into())),
                }
                cx.notify();
            })
            .ok();
            answer
        })
    }

    pub(crate) fn revert(&self, cx: &mut Context<Self>) -> Task<Result<(), Failure>> {
        let call = self.device.configure::<Revert, ()>(());
        cx.spawn(async move |this, cx| {
            let answer = call.await;
            match &answer {
                Ok(()) => {
                    if let Ok(reload) = this.update(cx, |store, cx| {
                        store.forget_changes(cx);
                        store.reload(cx)
                    }) {
                        reload.await;
                    }
                }
                Err(error) => {
                    this.update(cx, |_, cx| {
                        cx.emit(StoreEvent::Failed(error.to_string().into()));
                    })
                    .ok();
                }
            }
            answer
        })
    }

    pub(crate) fn activate(&self, profile: Slot, cx: &mut Context<Self>) -> Task<()> {
        let call = self.device.configure::<ActivateProfile, ()>(profile);
        cx.spawn(async move |this, cx| {
            if let Err(error) = call.await {
                this.update(cx, |_, cx| {
                    cx.emit(StoreEvent::Failed(error.to_string().into()));
                })
                .ok();
            }
            if let Ok(reload) = this.update(cx, |store, cx| store.reload_profiles(cx)) {
                reload.await;
            }
        })
    }

    pub(crate) fn create_profile(
        &self,
        namespace: Namespace,
        cx: &mut Context<Self>,
    ) -> Task<Option<Slot>> {
        let call = self.device.configure::<CreateProfile, Slot>(namespace);
        cx.spawn(async move |this, cx| {
            let created = call.await;
            if let Err(error) = &created {
                this.update(cx, |_, cx| {
                    cx.emit(StoreEvent::Failed(error.to_string().into()));
                })
                .ok();
            }
            if let Ok(reload) = this.update(cx, |store, cx| store.reload_profiles(cx)) {
                reload.await;
            }
            created.ok()
        })
    }

    pub(crate) fn select_linked(
        &self,
        selection: Selection,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), Failure>> {
        let call = self.device.configure::<SelectLinkedProfile, ()>(selection);
        cx.spawn(async move |this, cx| {
            let answer = call.await;
            if answer.is_ok()
                && let Ok(reload) = this.update(cx, |store, cx| {
                    if let Some(keyboard) = &mut store.keyboard {
                        keyboard.selected = selection.profile;
                    }
                    store.reload_profiles(cx)
                })
            {
                reload.await;
            }
            answer
        })
    }

    pub(crate) fn set_app_links(
        &mut self,
        profile: Slot,
        links: Vec<apps::Link>,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), Failure>> {
        let before = self.known_links(profile);
        self.history.record_replays(
            SetAppLink::PATH,
            Scope::Profile(profile),
            &before,
            &links,
            app_links_writer(profile, before.clone()),
            app_links_writer(profile, links.clone()),
        );
        cx.notify();
        self.write_app_links(profile, links, cx)
    }

    pub(crate) fn write_app_links(
        &self,
        profile: Slot,
        links: Vec<apps::Link>,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), Failure>> {
        let known = self.known_links(profile);
        let device = self.device.clone();
        cx.spawn(async move |this, cx| {
            let written = replace_app_links(&device, profile, &known, &links).await;
            this.update(cx, |store, cx| {
                match &written {
                    Ok(()) => {
                        if let Some(linked) = store
                            .keyboard
                            .as_mut()
                            .and_then(|keyboard| keyboard.linked_profile_mut(profile))
                        {
                            linked.links = links;
                        }
                    }
                    Err(error) => cx.emit(StoreEvent::Failed(error.to_string().into())),
                }
                cx.notify();
            })
            .ok();
            written
        })
    }

    fn known_links(&self, profile: Slot) -> Vec<apps::Link> {
        self.keyboard
            .as_ref()
            .and_then(|keyboard| keyboard.linked_profile(profile))
            .map_or_else(Vec::new, |linked| linked.links.clone())
    }

    pub(crate) fn delete_profile(&mut self, profile: Slot, cx: &mut Context<Self>) -> Task<()> {
        self.history.clear();
        let call = self.device.configure::<DeleteProfile, ()>(profile);
        cx.spawn(async move |this, cx| {
            if let Err(error) = call.await {
                this.update(cx, |_, cx| {
                    cx.emit(StoreEvent::Failed(error.to_string().into()));
                })
                .ok();
            }
            if let Ok(reload) = this.update(cx, |store, cx| store.reload_profiles(cx)) {
                reload.await;
            }
        })
    }

    pub(crate) fn forget_changes(&mut self, cx: &mut Context<Self>) {
        self.history.clear();
        if let Some(keyboard) = &mut self.keyboard {
            keyboard.unsaved = false;
        }
        cx.notify();
    }

    pub(crate) fn clear_history(&mut self, cx: &mut Context<Self>) {
        self.history.clear();
        cx.notify();
    }

    pub(crate) fn set_profile_info(
        &mut self,
        profile: Slot,
        info: profile::Info,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), Failure>> {
        let after = Change {
            profile,
            value: info.clone(),
        };
        let before = self
            .keyboard
            .as_ref()
            .and_then(|keyboard| keyboard.info(profile))
            .cloned()
            .map(|value| Change { profile, value });
        let applied = match before {
            Some(before) => {
                self.change::<SetProfileInfo>(&before, after, Scope::Profile(profile), cx)
            }
            None => self.apply::<SetProfileInfo>(after, Scope::Profile(profile), cx),
        };
        cx.spawn(async move |this, cx| {
            let answer = applied.await;
            if answer.is_ok() {
                this.update(cx, |store, cx| {
                    if let Some(entry) = store
                        .keyboard
                        .as_mut()
                        .and_then(|keyboard| keyboard.info_mut(profile))
                    {
                        *entry = info;
                    }
                    cx.notify();
                })
                .ok();
            }
            answer
        })
    }

    pub(crate) fn reload(&self, cx: &mut Context<Self>) -> Task<()> {
        let Some(keyboard) = &self.keyboard else {
            return Task::ready(());
        };
        let snapshot = load_snapshot(
            self.device.clone(),
            keyboard.name.to_string(),
            keyboard.version,
        );
        cx.spawn(async move |this, cx| {
            let loaded = snapshot.await;
            this.update(cx, |store, cx| {
                match loaded {
                    Ok(mut keyboard) => {
                        keyboard.unsaved = store.keyboard.as_ref().is_some_and(|kept| kept.unsaved);
                        store.keyboard = Some(keyboard);
                        store.link = Link::Connected;
                        cx.emit(StoreEvent::Reloaded);
                    }
                    Err(error) => cx.emit(StoreEvent::Failed(error.to_string().into())),
                }
                cx.notify();
            })
            .ok();
        })
    }

    pub(crate) fn reload_switches(&self, cx: &mut Context<Self>) -> Task<()> {
        let call = self.device.configure::<GetSwitches, switches::Settings>(());
        cx.spawn(async move |this, cx| {
            if let Ok(switches) = call.await {
                this.update(cx, |store, cx| {
                    if let Some(keyboard) = &mut store.keyboard {
                        keyboard.switches = Some(switches);
                    }
                    cx.notify();
                })
                .ok();
            }
        })
    }

    pub(crate) fn set_keymap(
        &mut self,
        layer: Layer,
        keymap: keys::Keymap,
        cx: &mut Context<Self>,
    ) {
        if let Some(keyboard) = &mut self.keyboard {
            keyboard.set_keymap(layer, keymap);
            cx.notify();
        }
    }

    fn on_presence(&mut self, presence: Presence, cx: &mut Context<Self>) {
        self.searching = false;
        match presence {
            Presence::Connected { name, version } => {
                self.link = Link::Loading;
                let snapshot = load_snapshot(self.device.clone(), name, version);
                cx.spawn(async move |this, cx| {
                    let loaded = snapshot.await;
                    this.update(cx, |store, cx| {
                        match loaded {
                            Ok(keyboard) => {
                                store.keyboard = Some(keyboard);
                                store.link = Link::Connected;
                                cx.emit(StoreEvent::Connected);
                            }
                            Err(error) => {
                                store.link = Link::Unreachable(error.to_string().into());
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            Presence::Absent(reason) => {
                let was_connected = self.keyboard.take().is_some();
                self.history.clear();
                self.link =
                    reason.map_or(Link::Searching, |reason| Link::Unreachable(reason.into()));
                if was_connected {
                    cx.emit(StoreEvent::Disconnected);
                }
            }
        }
        cx.notify();
    }

    fn poll_profiles(&self, cx: &mut Context<Self>) -> Option<Task<()>> {
        let keyboard = self.keyboard.as_ref()?;
        let known = (
            keyboard.active,
            keyboard.profiles.len(),
            keyboard.linked.len(),
            keyboard.selected,
        );
        let call = self.device.configure::<GetProfiles, profile::Summary>(());
        Some(cx.spawn(async move |this, cx| {
            let Ok(summary) = call.await else {
                return;
            };
            let summary_state = (
                summary.active,
                usize::from(summary.count),
                usize::from(summary.linked),
                summary.selected,
            );
            if summary_state != known
                && let Ok(reload) = this.update(cx, |store, cx| store.reload_profiles(cx))
            {
                reload.await;
            }
        }))
    }

    fn reload_profiles(&self, cx: &mut Context<Self>) -> Task<()> {
        let device = self.device.clone();
        cx.spawn(async move |this, cx| {
            let loaded = load_profiles(&device).await;
            this.update(cx, |store, cx| {
                match loaded {
                    Ok(ProfileSet {
                        active,
                        profiles,
                        linked,
                        selected,
                        legends,
                        fn_keymaps,
                    }) => {
                        if let Some(keyboard) = &mut store.keyboard {
                            let changed = keyboard.active != active;
                            keyboard.active = active;
                            keyboard.profiles = profiles;
                            keyboard.linked = linked;
                            keyboard.selected = selected;
                            keyboard.legends = legends;
                            keyboard.fn_keymaps = fn_keymaps;
                            if changed {
                                keyboard.unsaved = false;
                            }
                            cx.emit(StoreEvent::ProfileChanged);
                        }
                    }
                    Err(error) => cx.emit(StoreEvent::Failed(error.to_string().into())),
                }
                cx.notify();
            })
            .ok();
        })
    }

    fn mark_changed(&mut self, scope: Scope, cx: &mut Context<Self>) {
        let Some(keyboard) = &mut self.keyboard else {
            return;
        };
        let stays_until_save = match scope {
            Scope::Profile(profile) => profile == keyboard.active,
            Scope::Keyboard => true,
            Scope::Runtime => false,
        };
        if stays_until_save && !keyboard.unsaved {
            keyboard.unsaved = true;
            cx.notify();
        }
    }
}

impl Keyboard {
    pub(crate) fn display_name(&self) -> SharedString {
        self.device_name
            .clone()
            .unwrap_or_else(|| self.name.clone())
    }

    pub(crate) fn key_count(&self) -> usize {
        self.capabilities.key_count()
    }

    pub(crate) fn columns(&self) -> usize {
        usize::from(self.capabilities.key_columns)
    }

    pub(crate) fn travel_mm(&self, key: usize) -> f32 {
        let factory = switches::Switch {
            model: u8::from(self.capabilities.edition.is_some()),
            travel_offset: 0,
        };
        let switch = self
            .switches
            .as_ref()
            .and_then(|switches| switches.of_key(key))
            .unwrap_or(factory);
        self.models
            .get(usize::from(switch.model))
            .map_or(FALLBACK_TRAVEL_MM, |entry| {
                f32::from(entry.travel.saturating_sub(switch.travel_offset))
                    * MILLIMETRES_PER_TRAVEL_STEP
            })
    }

    pub(crate) fn distance_mm(&self, key: usize, distance: Distance) -> f32 {
        f32::from(distance.fraction()) / f32::from(u16::MAX) * self.travel_mm(key)
    }

    pub(crate) fn distance_from_mm(&self, key: usize, millimetres: f32) -> Distance {
        let travel = self.travel_mm(key);
        let fraction = (millimetres / travel).clamp(0.0, 1.0) * f32::from(u16::MAX);
        Distance::from_fraction(fraction_to_u16(fraction).max(1))
    }

    pub(crate) fn profile_name(&self, profile: Slot) -> SharedString {
        self.info(profile).map_or_else(
            || format!("Profile {}", u16::from(profile.index()) + 1).into(),
            |info| info.name.as_str().to_owned().into(),
        )
    }

    pub(crate) fn info(&self, profile: Slot) -> Option<&profile::Info> {
        if profile.is_linked() {
            self.linked_profile(profile).map(|linked| &linked.info)
        } else {
            self.profiles.get(usize::from(profile.get()))
        }
    }

    pub(crate) fn linked_profile(&self, profile: Slot) -> Option<&Linked> {
        profile
            .is_linked()
            .then(|| self.linked.get(usize::from(profile.index())))
            .flatten()
    }

    fn info_mut(&mut self, profile: Slot) -> Option<&mut profile::Info> {
        if profile.is_linked() {
            self.linked_profile_mut(profile)
                .map(|linked| &mut linked.info)
        } else {
            self.profiles.get_mut(usize::from(profile.get()))
        }
    }

    fn linked_profile_mut(&mut self, profile: Slot) -> Option<&mut Linked> {
        if profile.is_linked() {
            self.linked.get_mut(usize::from(profile.index()))
        } else {
            None
        }
    }

    pub(crate) fn has_key(&self, key: usize) -> bool {
        match &self.geometry {
            Some(geometry) => geometry.get(key).is_some_and(Option::is_some),
            None => self
                .legends
                .get(key)
                .is_some_and(|action| *action != Action::None),
        }
    }

    pub(crate) fn shortcut(&self, target: Action) -> Vec<Action> {
        combinations(
            &self.legends,
            &self.fn_keymaps,
            target,
            Layer::Base,
            &mut Vec::new(),
        )
        .into_iter()
        .next()
        .unwrap_or_default()
    }

    fn set_keymap(&mut self, layer: Layer, keymap: keys::Keymap) {
        if layer == Layer::Base {
            self.legends = keymap;
        } else if let Some(entry) = self
            .fn_keymaps
            .iter_mut()
            .find(|(fn_layer, _)| *fn_layer == layer)
        {
            entry.1 = keymap;
        } else {
            self.fn_keymaps.push((layer, keymap));
        }
    }
}

fn fraction_to_u16(fraction: f32) -> u16 {
    let rounded = fraction.round();
    if rounded <= 0.0 {
        0
    } else if rounded >= f32::from(u16::MAX) {
        u16::MAX
    } else {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is rounded and lies within 0..=u16::MAX"
        )]
        let value = rounded as u16;
        value
    }
}

fn combinations(
    legends: &keys::Keymap,
    fn_keymaps: &[(Layer, keys::Keymap)],
    target: Action,
    layer: Layer,
    opened: &mut Vec<Layer>,
) -> Vec<Vec<Action>> {
    let keymap = if layer == Layer::Base {
        Some(legends)
    } else {
        fn_keymaps
            .iter()
            .find(|(fn_layer, _)| *fn_layer == layer)
            .map(|(_, keymap)| keymap)
    };
    let Some(keymap) = keymap else {
        return Vec::new();
    };
    let position = |action: Action| keymap.iter().position(|bound| *bound == action);
    let pressed = |key: usize| {
        legends
            .get(key)
            .copied()
            .filter(|action| *action != Action::None)
    };
    let mut found: Vec<Vec<Action>> = position(target)
        .and_then(pressed)
        .map(|key| vec![key])
        .into_iter()
        .collect();
    let reachable: Vec<Layer> = FN_LAYERS
        .into_iter()
        .filter(|fn_layer| {
            *fn_layer != layer
                && !opened.contains(fn_layer)
                && position(Action::Layer(*fn_layer)).is_some()
        })
        .collect();
    for fn_layer in reachable {
        if !opened.contains(&fn_layer) {
            opened.push(fn_layer);
        }
        let deeper = combinations(legends, fn_keymaps, target, fn_layer, opened);
        if let Some(switch) = position(Action::Layer(fn_layer)).and_then(pressed) {
            found.extend(
                deeper
                    .into_iter()
                    .map(|rest| std::iter::once(switch).chain(rest).collect()),
            );
        }
    }
    found
}

async fn load_snapshot(
    device: Device,
    name: String,
    version: Version,
) -> Result<Keyboard, Failure> {
    let capabilities = device
        .configure::<GetCapabilities, Capabilities>(())
        .await?;
    let geometry = device
        .configure::<GetKeyGeometry, geometry::Keys>(())
        .await
        .ok();
    let layout = device
        .configure::<GetLayout, LayoutState>(())
        .await
        .ok()
        .map(|layout| layout.active);
    let switches = device
        .configure::<GetSwitches, switches::Settings>(())
        .await
        .ok();
    let models = device
        .configure::<GetSwitchModels, switches::Models>(())
        .await
        .unwrap_or_default();
    let device_name = device
        .configure::<GetDeviceName, device::Name>(())
        .await
        .ok()
        .filter(|name| !name.is_empty())
        .map(|name| SharedString::from(name.as_str().to_owned()));
    let ProfileSet {
        active,
        profiles,
        linked,
        selected,
        legends,
        fn_keymaps,
    } = load_profiles(&device).await?;
    Ok(Keyboard {
        name: name.into(),
        device_name,
        version,
        capabilities,
        geometry,
        layout,
        profiles,
        linked,
        selected,
        active,
        legends,
        fn_keymaps,
        switches,
        models,
        unsaved: false,
    })
}

async fn load_profiles(device: &Device) -> Result<ProfileSet, Failure> {
    let summary = device
        .configure::<GetProfiles, profile::Summary>(())
        .await?;
    let mut profiles = Vec::with_capacity(usize::from(summary.count));
    for index in 0..summary.count {
        profiles.push(
            device
                .configure::<GetProfileInfo, profile::Info>(Slot::new(index))
                .await?,
        );
    }
    let mut linked = Vec::with_capacity(usize::from(summary.linked));
    for index in 0..summary.linked {
        let slot = Slot::linked(index);
        let info = device
            .configure::<GetProfileInfo, profile::Info>(slot)
            .await?;
        let links = load_app_links(device, slot).await?;
        linked.push(Linked { info, links });
    }
    let legends = device
        .configure::<GetKeymap, keys::Keymap>(LayerRef {
            profile: summary.active,
            layer: Layer::Base,
        })
        .await?;
    let mut fn_keymaps = Vec::with_capacity(FN_LAYERS.len());
    for layer in FN_LAYERS {
        let keymap = device
            .configure::<GetKeymap, keys::Keymap>(LayerRef {
                profile: summary.active,
                layer,
            })
            .await?;
        fn_keymaps.push((layer, keymap));
    }
    Ok(ProfileSet {
        active: summary.active,
        profiles,
        linked,
        selected: summary.selected,
        legends,
        fn_keymaps,
    })
}

async fn load_app_links(device: &Device, profile: Slot) -> Result<Vec<apps::Link>, Failure> {
    let count = device.configure::<GetAppLinkCount, u8>(profile).await?;
    let mut links = Vec::with_capacity(usize::from(count));
    for index in 0..count {
        links.push(
            device
                .configure::<GetAppLink, apps::Link>(apps::LinkRef { profile, index })
                .await?,
        );
    }
    Ok(links)
}

async fn replace_app_links(
    device: &Device,
    profile: Slot,
    known: &[apps::Link],
    links: &[apps::Link],
) -> Result<(), Failure> {
    let count = device.configure::<GetAppLinkCount, u8>(profile).await?;
    for (index, link) in (0..apps::MAX_LINKS).zip(links) {
        if index < count && known.get(usize::from(index)) == Some(link) {
            continue;
        }
        let change = apps::LinkChange {
            profile,
            index,
            link: link.clone(),
        };
        device.configure::<SetAppLink, ()>(change).await?;
    }
    let kept = u8::try_from(links.len()).unwrap_or(apps::MAX_LINKS);
    for index in (kept..count).rev() {
        device
            .configure::<DeleteAppLink, ()>(apps::LinkRef { profile, index })
            .await?;
    }
    Ok(())
}

fn app_links_writer(profile: Slot, links: Vec<apps::Link>) -> Replay {
    Rc::new(move |store, cx| store.write_app_links(profile, links.clone(), cx))
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::keys::Function;

    use super::*;

    const KEY_A: Action = Action::Keyboard(0x04);
    const KEY_B: Action = Action::Keyboard(0x05);
    const ONE: Action = Action::Keyboard(0x1E);
    const TWO: Action = Action::Keyboard(0x1F);
    const FN_ONE: Action = Action::Layer(Layer::One);
    const FN_TWO: Action = Action::Layer(Layer::Two);
    const MODE: Action = Action::Function(Function::ToggleModeProfile);

    fn keymap(actions: &[Action]) -> keys::Keymap {
        actions.iter().copied().collect()
    }

    fn profile(slot: u8) -> Action {
        Action::Function(Function::Profile(Slot::new(slot)))
    }

    fn found(
        legends: &keys::Keymap,
        fn_keymaps: &[(Layer, keys::Keymap)],
        target: Action,
    ) -> Vec<Vec<Action>> {
        combinations(legends, fn_keymaps, target, Layer::Base, &mut Vec::new())
    }

    #[test]
    fn a_key_on_the_main_layer_is_its_own_shortcut() {
        let legends = keymap(&[KEY_A, MODE]);
        assert_eq!(found(&legends, &[], MODE), [[MODE]]);
    }

    #[test]
    fn a_key_on_a_fn_layer_is_reached_by_holding_the_fn_key() {
        let legends = keymap(&[ONE, TWO, FN_ONE]);
        let fn_keymaps = [(Layer::One, keymap(&[profile(0), profile(1), Action::None]))];
        assert_eq!(found(&legends, &fn_keymaps, profile(1)), [[FN_ONE, TWO]]);
        assert_eq!(
            found(&legends, &fn_keymaps, profile(2)),
            Vec::<Vec<Action>>::new()
        );
    }

    #[test]
    fn deeper_fn_layers_are_reached_through_the_keys_that_open_them() {
        let legends = keymap(&[KEY_A, FN_ONE, KEY_B]);
        let fn_keymaps = [
            (Layer::One, keymap(&[Action::None, Action::None, FN_TWO])),
            (
                Layer::Two,
                keymap(&[profile(0), Action::None, Action::None]),
            ),
        ];
        assert_eq!(
            found(&legends, &fn_keymaps, profile(0)),
            [[FN_ONE, KEY_B, KEY_A]]
        );
    }

    #[test]
    fn the_main_layer_comes_before_the_fn_layers() {
        let legends = keymap(&[MODE, KEY_A, FN_ONE]);
        let fn_keymaps = [(Layer::One, keymap(&[Action::None, MODE, Action::None]))];
        assert_eq!(
            found(&legends, &fn_keymaps, MODE),
            [vec![MODE], vec![FN_ONE, KEY_A]]
        );
    }

    #[test]
    fn a_position_without_a_main_layer_key_cannot_be_pressed() {
        let legends = keymap(&[Action::None, FN_ONE]);
        let fn_keymaps = [(Layer::One, keymap(&[profile(0), Action::None]))];
        assert_eq!(
            found(&legends, &fn_keymaps, profile(0)),
            Vec::<Vec<Action>>::new()
        );
    }
}
