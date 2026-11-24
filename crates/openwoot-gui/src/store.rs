use std::time::Duration;

use async_channel::Receiver;
use gpui_kit::{Context, EventEmitter, SharedString, Task};
use openwoot_protocol::{
    Endpoint, Version,
    configuration::{Capabilities, Rejection},
    device::Layout,
    endpoints::{
        ActivateProfile, CreateProfile, DeleteProfile, GetCapabilities, GetKeyGeometry, GetKeymap,
        GetLayout, GetProfileInfo, GetProfiles, GetSwitchModels, GetSwitches, Revert, Save,
        SetProfileInfo,
    },
    geometry,
    keys::{self, Action, Distance, Layer},
    profile::{self, Change, LayerRef, Slot},
    switches,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::device::{Device, Failure, Presence};

const PROFILE_POLL_INTERVAL: Duration = Duration::from_secs(1);
const MILLIMETRES_PER_TRAVEL_STEP: f32 = 0.05;
const FALLBACK_TRAVEL_MM: f32 = 4.0;

pub(crate) struct Store {
    device: Device,
    link: Link,
    keyboard: Option<Keyboard>,
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
    pub(crate) version: Version,
    pub(crate) capabilities: Capabilities,
    pub(crate) geometry: Option<geometry::Keys>,
    pub(crate) layout: Option<Layout>,
    pub(crate) profiles: Vec<profile::Info>,
    pub(crate) active: Slot,
    pub(crate) legends: keys::Keymap,
    pub(crate) switches: Option<switches::Settings>,
    pub(crate) models: switches::Models,
    pub(crate) unsaved: bool,
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
                    if let Ok(reload) = this.update(cx, |store, cx| store.reload(cx)) {
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

    pub(crate) fn create_profile(&self, cx: &mut Context<Self>) -> Task<Option<Slot>> {
        let call = self.device.configure::<CreateProfile, Slot>(());
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

    pub(crate) fn delete_profile(&self, profile: Slot, cx: &mut Context<Self>) -> Task<()> {
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

    pub(crate) fn set_profile_info(
        &self,
        profile: Slot,
        info: profile::Info,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), Failure>> {
        let applied = self.apply::<SetProfileInfo>(
            Change {
                profile,
                value: info.clone(),
            },
            Scope::Profile(profile),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let answer = applied.await;
            if answer.is_ok() {
                this.update(cx, |store, cx| {
                    if let Some(entry) = store
                        .keyboard
                        .as_mut()
                        .and_then(|keyboard| keyboard.profiles.get_mut(usize::from(profile.get())))
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
                    Ok(keyboard) => {
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

    pub(crate) fn set_legends(&mut self, legends: keys::Keymap, cx: &mut Context<Self>) {
        if let Some(keyboard) = &mut self.keyboard {
            keyboard.legends = legends;
            cx.notify();
        }
    }

    fn on_presence(&mut self, presence: Presence, cx: &mut Context<Self>) {
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
        let known = (keyboard.active, keyboard.profiles.len());
        let call = self.device.configure::<GetProfiles, profile::Summary>(());
        Some(cx.spawn(async move |this, cx| {
            let Ok(summary) = call.await else {
                return;
            };
            if (summary.active, usize::from(summary.count)) != known
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
                    Ok((active, profiles, legends)) => {
                        if let Some(keyboard) = &mut store.keyboard {
                            let changed = keyboard.active != active;
                            keyboard.active = active;
                            keyboard.profiles = profiles;
                            keyboard.legends = legends;
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
        self.profiles.get(usize::from(profile.get())).map_or_else(
            || format!("Profile {}", u16::from(profile.get()) + 1).into(),
            |info| info.name.as_str().to_owned().into(),
        )
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
    let layout = device.configure::<GetLayout, Layout>(()).await.ok();
    let switches = device
        .configure::<GetSwitches, switches::Settings>(())
        .await
        .ok();
    let models = device
        .configure::<GetSwitchModels, switches::Models>(())
        .await
        .unwrap_or_default();
    let (active, profiles, legends) = load_profiles(&device).await?;
    Ok(Keyboard {
        name: name.into(),
        version,
        capabilities,
        geometry,
        layout,
        profiles,
        active,
        legends,
        switches,
        models,
        unsaved: false,
    })
}

async fn load_profiles(
    device: &Device,
) -> Result<(Slot, Vec<profile::Info>, keys::Keymap), Failure> {
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
    let legends = device
        .configure::<GetKeymap, keys::Keymap>(LayerRef {
            profile: summary.active,
            layer: Layer::Base,
        })
        .await?;
    Ok((summary.active, profiles, legends))
}
