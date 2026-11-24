mod file;
mod parts;
mod preset;

use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
};

use gpui_kit::{
    AnyElement, AppContext as _, AsyncApp, Context, Div, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, PathPromptOptions, Render, Rgba,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity,
    Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, IconName, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariant, ButtonVariants as _},
        color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
        h_flex,
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu as _, PopupMenuItem},
        progress::Progress,
        tag::Tag,
        v_flex,
    },
    div, px,
};
use openwoot_protocol::{
    endpoints::{
        GetGamepadMapping, GetKeyOverrides, GetKeySettings, GetKeymap, SetGamepadMapping,
        SetKeyOverrides, SetKeySettings,
    },
    gamepad, heapless,
    keys::{self, Layer},
    lighting::Rgb,
    profile::{self, Change, LayerRef, NAME_LEN, Slot},
};

use self::{
    file::{EXTENSION, FileError, ProfileFile},
    parts::{Parts, apply, fetch},
    preset::{Movement, Preset},
};
use super::Page;
use crate::{
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
};

type Name = heapless::String<NAME_LEN>;
type Outcome = Result<SharedString, SharedString>;

const ROW_HEIGHT: f32 = 64.0;
const BADGE_SIZE: f32 = 40.0;
const NAME_FIELD_WIDTH: f32 = 320.0;

pub(crate) struct ProfilesPage {
    store: Entity<Store>,
    editing: Option<Slot>,
    name: Entity<InputState>,
    color: Entity<ColorPickerState>,
    name_problem: Option<SharedString>,
    job: Option<Job>,
    import: Option<Result<Import, SharedString>>,
    _subscriptions: Vec<Subscription>,
}

struct Job {
    title: SharedString,
    done: usize,
    total: usize,
    outcome: Option<Outcome>,
}

struct Import {
    path: PathBuf,
    file: ProfileFile,
    warnings: Vec<SharedString>,
}

#[derive(Clone, Copy)]
enum Target {
    NewProfile,
    Active(Slot),
}

impl ProfilesPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Profile name"));
        let color = cx.new(|cx| ColorPickerState::new(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected | StoreEvent::ProfileChanged | StoreEvent::Reloaded => {
                    this.sync_editor(window, cx);
                }
                StoreEvent::Disconnected => {
                    this.editing = None;
                    this.import = None;
                    cx.notify();
                }
                StoreEvent::Failed(_) => {}
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &name,
                window,
                |this, _, event: &InputEvent, _, cx| match event {
                    InputEvent::Change => this.check_name(cx),
                    InputEvent::PressEnter { .. } | InputEvent::Blur => this.rename(cx),
                    InputEvent::Focus => {}
                },
            ),
            cx.subscribe_in(
                &color,
                window,
                |this, _, event: &ColorPickerEvent, _, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        this.recolor(rgb_of(*color), cx);
                    }
                },
            ),
        ];
        let mut page = Self {
            store,
            editing: None,
            name,
            color,
            name_problem: None,
            job: None,
            import: None,
            _subscriptions: subscriptions,
        };
        page.sync_editor(window, cx);
        page
    }

    fn busy(&self) -> bool {
        self.job.as_ref().is_some_and(|job| job.outcome.is_none())
    }

    fn edited(&self, cx: &Context<Self>) -> Option<(Slot, profile::Info)> {
        let keyboard = self.store.read(cx).keyboard()?;
        let slot = self
            .editing
            .filter(|slot| usize::from(slot.get()) < keyboard.profiles.len())
            .unwrap_or(keyboard.active);
        let info = keyboard.profiles.get(usize::from(slot.get()))?.clone();
        Some((slot, info))
    }

    fn sync_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.name_problem = None;
        if let Some((_, info)) = self.edited(cx) {
            self.name.update(cx, |input, cx| {
                input.set_value(info.name.as_str().to_owned(), window, cx);
            });
            self.color.update(cx, |picker, cx| {
                picker.set_value(hsla_of(info.color), window, cx);
            });
        }
        cx.notify();
    }

    fn edit(&mut self, slot: Slot, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = Some(slot);
        self.sync_editor(window, cx);
    }

    fn check_name(&mut self, cx: &mut Context<Self>) {
        let text = self.name.read(cx).value();
        self.name_problem = profile_name(&text).err().map(SharedString::from);
        cx.notify();
    }

    fn rename(&mut self, cx: &mut Context<Self>) {
        let Some((slot, info)) = self.edited(cx) else {
            return;
        };
        let text = self.name.read(cx).value();
        match profile_name(&text) {
            Ok(name) if name != info.name => {
                self.name_problem = None;
                let info = profile::Info {
                    name,
                    color: info.color,
                };
                self.store
                    .update(cx, |store, cx| store.set_profile_info(slot, info, cx))
                    .detach();
            }
            Ok(_) => self.name_problem = None,
            Err(problem) => self.name_problem = Some(problem.into()),
        }
        cx.notify();
    }

    fn recolor(&mut self, color: Rgb, cx: &mut Context<Self>) {
        let Some((slot, info)) = self.edited(cx) else {
            return;
        };
        if info.color == color {
            return;
        }
        let info = profile::Info {
            name: info.name,
            color,
        };
        self.store
            .update(cx, |store, cx| store.set_profile_info(slot, info, cx))
            .detach();
    }

    fn activate(&self, slot: Slot, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.activate(slot, cx))
            .detach();
    }

    fn confirm_delete(&self, slot: Slot, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let name = keyboard.profile_name(slot);
        let handover = (keyboard.active == slot)
            .then(|| {
                slot.get()
                    .checked_sub(1)
                    .map(|previous| keyboard.profile_name(Slot::new(previous)))
            })
            .flatten();
        let store = self.store.clone();
        let page = cx.entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let store = store.clone();
            let page = page.clone();
            let description = handover.as_ref().map_or_else(
                || "This cannot be undone.".to_owned(),
                |next| format!("This cannot be undone. “{next}” becomes active."),
            );
            alert
                .title(format!("Delete “{name}”?"))
                .description(description)
                .confirm()
                .ok_text("Delete")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    page.update(cx, |page, _| {
                        if page.editing == Some(slot) {
                            page.editing = None;
                        }
                    });
                    store
                        .update(cx, |store, cx| store.delete_profile(slot, cx))
                        .detach();
                    true
                })
        });
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
            outcome: None,
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

    fn finish(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        if let Some(job) = &mut self.job {
            if outcome.is_ok() {
                job.done = job.total;
            }
            job.outcome = Some(outcome);
        }
        cx.notify();
    }

    fn create(&mut self, preset: Preset, cx: &mut Context<Self>) {
        if !self.begin(
            format!("Creating “{}”", preset.name()),
            preset_steps(preset),
            cx,
        ) {
            return;
        }
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let mut step = progress(&this);
            let outcome = create_from_preset(&store, preset, cx, &mut step).await;
            this.update(cx, |page, cx| page.finish(outcome, cx)).ok();
        })
        .detach();
    }

    fn duplicate(&mut self, source: Slot, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let Some(info) = keyboard.profiles.get(usize::from(source.get())).cloned() else {
            return;
        };
        let lightbar = keyboard.capabilities.lightbar_leds > 0;
        let total = 2 * Parts::steps(lightbar) + 2;
        if !self.begin(format!("Duplicating “{}”", info.name), total, cx) {
            return;
        }
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let mut step = progress(&this);
            let outcome = Box::pin(duplicate_profile(
                &store, source, info, lightbar, cx, &mut step,
            ))
            .await;
            this.update(cx, |page, cx| page.finish(outcome, cx)).ok();
        })
        .detach();
    }

    fn export(&self, slot: Slot, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let Some(info) = keyboard.profiles.get(usize::from(slot.get())).cloned() else {
            return;
        };
        let lightbar = keyboard.capabilities.lightbar_leds > 0;
        let keyboard_name = keyboard.name.to_string();
        let suggested = format!("{}.{EXTENSION}", file_stem(&info.name));
        let chosen = cx.prompt_for_new_path(&home_directory(), Some(&suggested));
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let path = match chosen.await {
                Ok(Ok(Some(path))) => path,
                Ok(Err(error)) => {
                    let problem = format!("could not open the file dialog: {error:#}");
                    this.update(cx, |page, cx| {
                        if page.begin("Exporting a profile", 1, cx) {
                            page.finish(Err(problem.into()), cx);
                        }
                    })
                    .ok();
                    return;
                }
                Ok(Ok(None)) | Err(_) => return,
            };
            let title = format!("Exporting “{}”", info.name);
            let total = Parts::steps(lightbar) + 1;
            if !this
                .update(cx, |page, cx| page.begin(title, total, cx))
                .unwrap_or(false)
            {
                return;
            }
            let mut step = progress(&this);
            let file = Export {
                slot,
                info,
                keyboard: keyboard_name,
                lightbar,
            };
            let outcome = Box::pin(export_profile(&store, file, path, cx, &mut step)).await;
            this.update(cx, |page, cx| page.finish(outcome, cx)).ok();
        })
        .detach();
    }

    fn choose_import(cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import profile".into()),
        });
        cx.spawn(async move |this, cx| {
            let path = match chosen.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Err(error)) => {
                    let problem = format!("could not open the file dialog: {error:#}");
                    this.update(cx, |page, cx| {
                        page.import = Some(Err(problem.into()));
                        cx.notify();
                    })
                    .ok();
                    None
                }
                Ok(Ok(None)) | Err(_) => None,
            };
            let Some(path) = path else {
                return;
            };
            let source = path.clone();
            let loaded = cx
                .background_executor()
                .spawn(async move { ProfileFile::load(&source) })
                .await;
            this.update(cx, |page, cx| page.review_import(path, loaded, cx))
                .ok();
        })
        .detach();
    }

    fn review_import(
        &mut self,
        path: PathBuf,
        loaded: Result<ProfileFile, FileError>,
        cx: &mut Context<Self>,
    ) {
        self.import = Some(self.store.read(cx).keyboard().map_or_else(
            || Err("Connect a keyboard to import a profile.".into()),
            |keyboard| {
                let file = loaded.map_err(|error| SharedString::from(error_chain(&error)))?;
                prepare_import(keyboard, path, file)
            },
        ));
        cx.notify();
    }

    fn import_into(&mut self, target: Target, cx: &mut Context<Self>) {
        let Some(Ok(import)) = &self.import else {
            return;
        };
        let file = import.file.clone();
        let lightbar = file.parts.lightbar.is_some();
        let title = match target {
            Target::NewProfile => format!("Importing “{}” as a new profile", file.info.name),
            Target::Active(_) => format!("Importing “{}” into the active profile", file.info.name),
        };
        let total = Parts::steps(lightbar) + 2;
        if !self.begin(title, total, cx) {
            return;
        }
        self.import = None;
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let mut step = progress(&this);
            let outcome = Box::pin(import_profile(&store, file, target, cx, &mut step)).await;
            this.update(cx, |page, cx| page.finish(outcome, cx)).ok();
        })
        .detach();
    }
}

fn progress(this: &WeakEntity<ProfilesPage>) -> impl FnMut(&mut AsyncApp) + '_ {
    move |cx| {
        this.update(cx, ProfilesPage::advance).ok();
    }
}

impl ProfilesPage {
    fn heading(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let busy = self.busy();
        let full = keyboard.profiles.len() >= usize::from(keyboard.capabilities.profiles);
        let page = cx.entity().downgrade();
        let new_profile = Button::new("new-profile")
            .outline()
            .small()
            .icon(Catalog::Plus)
            .label("New profile")
            .dropdown_caret(true)
            .disabled(busy || full)
            .dropdown_menu(move |menu, _, _| {
                Preset::ALL.into_iter().fold(menu, |menu, preset| {
                    let page = page.clone();
                    menu.item(PopupMenuItem::new(preset.name()).on_click(move |_, _, cx| {
                        page.update(cx, |page, cx| page.create(preset, cx)).ok();
                    }))
                })
            });
        panel::heading(
            Page::Profiles.title(),
            None,
            h_flex()
                .gap_2()
                .child(
                    Button::new("import-profile")
                        .outline()
                        .small()
                        .icon(Catalog::Download)
                        .label("Import profile")
                        .disabled(busy)
                        .on_click(cx.listener(|_, _, _, cx| Self::choose_import(cx))),
                )
                .child(div().child(new_profile)),
            cx,
        )
    }

    fn job_section(job: &Job, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let percent = if job.total == 0 {
            100.0
        } else {
            percentage(job.done, job.total)
        };
        let outcome = job.outcome.as_ref().map(|outcome| match outcome {
            Ok(message) => div()
                .text_sm()
                .text_color(theme.success)
                .child(message.clone()),
            Err(problem) => div()
                .text_sm()
                .text_color(theme.danger)
                .child(problem.clone()),
        });
        panel::section(job.title.clone(), None, cx)
            .child(Progress::new("profile-job").value(percent))
            .children(outcome)
            .children(job.outcome.is_some().then(|| {
                h_flex().justify_end().child(
                    Button::new("dismiss-job")
                        .ghost()
                        .small()
                        .label("Dismiss")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.job = None;
                            cx.notify();
                        })),
                )
            }))
    }

    fn onboard_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let edited = self.edited(cx).map(|(slot, _)| slot);
        let slots = usize::from(keyboard.capabilities.profiles);
        let count = keyboard.profiles.len();
        let cells: Vec<AnyElement> = (0..slots)
            .map(|index| {
                let profile = u8::try_from(index)
                    .ok()
                    .map(Slot::new)
                    .zip(keyboard.profiles.get(index));
                match profile {
                    Some((slot, info)) => ProfileRow {
                        index,
                        slot,
                        info,
                        count,
                        slots,
                        active: keyboard.active == slot,
                        selected: edited == Some(slot),
                        busy: self.busy(),
                    }
                    .render(cx)
                    .into_any_element(),
                    None => empty_slot(cx).into_any_element(),
                }
            })
            .collect();
        let mut rows = v_flex().gap_3();
        let mut cells = cells.into_iter();
        while let Some(first) = cells.next() {
            let second = cells.next();
            rows = rows.child(
                h_flex()
                    .gap_3()
                    .child(div().flex_1().min_w_0().child(first))
                    .child(div().flex_1().min_w_0().children(second)),
            );
        }
        panel::section(
            format!("Onboard profiles · {count}/{slots}"),
            Some("Stored on the keyboard and available on any computer.".into()),
            cx,
        )
        .child(rows)
    }

    fn editor_section(&self, cx: &Context<Self>) -> Div {
        let Some((slot, _)) = self.edited(cx) else {
            return div();
        };
        let busy = self.busy();
        panel::section(
            format!("Edit profile {}", u16::from(slot.get()) + 1),
            None,
            cx,
        )
        .child(
            h_flex()
                .items_start()
                .gap_6()
                .child(
                    div().w(px(NAME_FIELD_WIDTH)).child(panel::field(
                        "Profile name",
                        v_flex()
                            .gap_1()
                            .child(Input::new(&self.name).disabled(busy))
                            .children(self.name_problem.clone().map(|problem| {
                                div().text_xs().text_color(cx.theme().danger).child(problem)
                            })),
                        cx,
                    )),
                )
                .child(panel::field("Colour", ColorPicker::new(&self.color), cx)),
        )
    }

    fn import_section(import: &Import, keyboard: &Keyboard, busy: bool, cx: &Context<Self>) -> Div {
        let full = keyboard.profiles.len() >= usize::from(keyboard.capabilities.profiles);
        let active = keyboard.active;
        let mut description = format!(
            "{}, made for {}.",
            file_label(&import.path),
            import.file.keyboard
        );
        for warning in &import.warnings {
            description.push(' ');
            description.push_str(warning);
        }
        panel::section(
            format!("Import “{}”?", import.file.info.name),
            Some(description.into()),
            cx,
        )
        .child(
            h_flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("import-cancel")
                        .ghost()
                        .small()
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.import = None;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("import-active")
                        .outline()
                        .small()
                        .label(format!("Replace “{}”", keyboard.profile_name(active)))
                        .tooltip("Kept until you save")
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.import_into(Target::Active(active), cx);
                        })),
                )
                .children((!full).then(|| {
                    Button::new("import-new")
                        .primary()
                        .small()
                        .label("Add as new")
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.import_into(Target::NewProfile, cx);
                        }))
                })),
        )
    }
}

struct ProfileRow<'profile> {
    index: usize,
    slot: Slot,
    info: &'profile profile::Info,
    count: usize,
    slots: usize,
    active: bool,
    selected: bool,
    busy: bool,
}

type RowAction = fn(&mut ProfilesPage, Slot, &mut Window, &mut Context<ProfilesPage>);

impl ProfileRow<'_> {
    fn render(&self, cx: &Context<ProfilesPage>) -> Stateful<Div> {
        let theme = cx.theme();
        let slot = self.slot;
        let border = if self.active {
            theme.primary
        } else if self.selected {
            theme.border
        } else {
            theme.secondary
        };
        h_flex()
            .id(("profile-row", self.index))
            .relative()
            .w_full()
            .h(px(ROW_HEIGHT))
            .pl_5()
            .pr_2()
            .gap_3()
            .rounded(theme.radius_lg)
            .bg(theme.secondary)
            .border_1()
            .border_color(border)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| this.edit(slot, window, cx)))
            .child(
                div()
                    .absolute()
                    .top_1()
                    .left_1()
                    .px_1()
                    .rounded_sm()
                    .bg(theme.group_box)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child((self.index + 1).to_string()),
            )
            .child(
                div()
                    .flex_none()
                    .size(px(BADGE_SIZE))
                    .rounded_full()
                    .border_2()
                    .border_color(hsla_of(self.info.color))
                    .p_1()
                    .child(
                        div()
                            .size_full()
                            .rounded_full()
                            .bg(hsla_of(self.info.color)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.info.name.as_str().to_owned()),
            )
            .children(self.active.then(|| Tag::primary().small().child("Active")))
            .child(self.menu(cx))
    }

    fn menu(&self, cx: &Context<ProfilesPage>) -> Div {
        let page = cx.entity().downgrade();
        let slot = self.slot;
        let active = self.active;
        let deletable = self.index > 0 && self.index + 1 == self.count;
        let full = self.count >= self.slots;
        div().child(
            Button::new(("profile-menu", self.index))
                .ghost()
                .small()
                .icon(IconName::EllipsisVertical)
                .disabled(self.busy)
                .dropdown_menu(move |menu, _, _| {
                    let item = |label: &'static str, enabled: bool, run: RowAction| {
                        menu_item(page.clone(), slot, label, enabled, run)
                    };
                    menu.item(item("Activate", !active, |page, slot, _, cx| {
                        page.activate(slot, cx);
                    }))
                    .item(item("Duplicate", !full, |page, slot, _, cx| {
                        page.duplicate(slot, cx);
                    }))
                    .item(item("Export…", true, |page, slot, _, cx| {
                        page.export(slot, cx);
                    }))
                    .separator()
                    .item(item(
                        "Delete…",
                        deletable,
                        |page, slot, window, cx| {
                            page.confirm_delete(slot, window, cx);
                        },
                    ))
                }),
        )
    }
}

fn menu_item(
    page: WeakEntity<ProfilesPage>,
    slot: Slot,
    label: &'static str,
    enabled: bool,
    run: RowAction,
) -> PopupMenuItem {
    PopupMenuItem::new(label)
        .disabled(!enabled)
        .on_click(move |_, window, cx| {
            page.update(cx, |page, cx| run(page, slot, window, cx)).ok();
        })
}

fn empty_slot(cx: &Context<ProfilesPage>) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .h(px(ROW_HEIGHT))
        .justify_center()
        .rounded(theme.radius_lg)
        .border_1()
        .border_dashed()
        .border_color(theme.border)
        .text_sm()
        .text_color(theme.muted_foreground)
        .child("Empty slot")
}

fn file_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

impl Render for ProfilesPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = panel::page();
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return page.child(panel::note("No keyboard", cx));
        };
        let busy = self.busy();
        let job = self.job.as_ref().map(|job| Self::job_section(job, cx));
        let import = self.import.as_ref().map(|import| match import {
            Err(problem) => div()
                .text_sm()
                .text_color(cx.theme().danger)
                .child(problem.clone()),
            Ok(import) => Self::import_section(import, &keyboard, busy, cx),
        });
        page.child(self.heading(&keyboard, cx))
            .children(job)
            .children(import)
            .child(self.onboard_section(&keyboard, cx))
            .child(self.editor_section(cx))
    }
}

struct Export {
    slot: Slot,
    info: profile::Info,
    keyboard: String,
    lightbar: bool,
}

const fn preset_steps(preset: Preset) -> usize {
    let mut steps = 2;
    if preset.changes_keys() || preset.changes_gamepad() {
        steps += 1;
    }
    if preset.changes_keys() {
        steps += 4;
    }
    if preset.changes_gamepad() {
        steps += 2;
    }
    steps
}

async fn create_profile(store: &Entity<Store>, cx: &mut AsyncApp) -> Result<Slot, SharedString> {
    store
        .update(cx, |store, cx| store.create_profile(cx))
        .await
        .ok_or_else(|| "the keyboard did not create a profile".into())
}

async fn set_info(
    store: &Entity<Store>,
    slot: Slot,
    info: profile::Info,
    cx: &mut AsyncApp,
) -> Result<(), SharedString> {
    store
        .update(cx, |store, cx| store.set_profile_info(slot, info, cx))
        .await
        .map_err(|error| error_chain(&error).into())
}

async fn create_from_preset(
    store: &Entity<Store>,
    preset: Preset,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let slot = create_profile(store, cx).await?;
    step(cx);
    let tuned = tune_preset(store, slot, preset, cx, step).await;
    let mut name = Name::new();
    name.push_str(preset.name()).ok();
    let info = profile::Info {
        name,
        color: preset.color(),
    };
    set_info(store, slot, info, cx).await?;
    step(cx);
    tuned.map_err(|problem| {
        format!(
            "profile {} was created, but the preset could not be applied: {problem}",
            u16::from(slot.get()) + 1
        )
    })?;
    Ok(format!("Created profile {}", u16::from(slot.get()) + 1).into())
}

async fn tune_preset(
    store: &Entity<Store>,
    slot: Slot,
    preset: Preset,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Result<(), String> {
    if !preset.changes_keys() && !preset.changes_gamepad() {
        return Ok(());
    }
    let describe = |error: crate::device::Failure| error_chain(&error);
    let scope = Scope::Profile(slot);
    let base = LayerRef {
        profile: slot,
        layer: Layer::Base,
    };
    let keymap = fetch::<GetKeymap, keys::Keymap>(store, base, cx)
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
    Ok(())
}

async fn duplicate_profile(
    store: &Entity<Store>,
    source: Slot,
    info: profile::Info,
    lightbar: bool,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let parts = Parts::read(store, source, lightbar, cx, step)
        .await
        .map_err(|error| format!("could not read “{}”: {}", info.name, error_chain(&error)))?;
    let slot = create_profile(store, cx).await?;
    step(cx);
    let number = u16::from(slot.get()) + 1;
    parts.write(store, slot, cx, step).await.map_err(|error| {
        format!(
            "could not copy “{}” into profile {number}, which now holds part of it: {}",
            info.name,
            error_chain(&error)
        )
    })?;
    let copy = profile::Info {
        name: copy_name(&info.name),
        color: info.color,
    };
    set_info(store, slot, copy, cx).await?;
    step(cx);
    Ok(format!("Duplicated into profile {number}").into())
}

async fn export_profile(
    store: &Entity<Store>,
    export: Export,
    path: PathBuf,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let parts = Parts::read(store, export.slot, export.lightbar, cx, step)
        .await
        .map_err(|error| error_chain(&error))?;
    let file = ProfileFile::new(export.keyboard, export.info, parts);
    let target = path.clone();
    cx.background_executor()
        .spawn(async move { file.save(&target) })
        .await
        .map_err(|error| error_chain(&error))?;
    step(cx);
    Ok(format!("Exported to {}", path.display()).into())
}

async fn import_profile(
    store: &Entity<Store>,
    file: ProfileFile,
    target: Target,
    cx: &mut AsyncApp,
    step: &mut impl FnMut(&mut AsyncApp),
) -> Outcome {
    let slot = match target {
        Target::NewProfile => create_profile(store, cx).await?,
        Target::Active(slot) => slot,
    };
    step(cx);
    let number = u16::from(slot.get()) + 1;
    file.parts
        .write(store, slot, cx, step)
        .await
        .map_err(|error| {
            format!(
                "could not import “{}” into profile {number}, which now holds part of it: {}",
                file.info.name,
                error_chain(&error)
            )
        })?;
    set_info(store, slot, file.info.clone(), cx).await?;
    step(cx);
    match target {
        Target::NewProfile => Ok(format!("Imported as profile {number}").into()),
        Target::Active(_) => {
            store.update(cx, |store, cx| store.reload(cx)).await;
            Ok("Imported into the active profile; save to keep it".into())
        }
    }
}

fn prepare_import(
    keyboard: &Keyboard,
    path: PathBuf,
    mut file: ProfileFile,
) -> Result<Import, SharedString> {
    let mut warnings = Vec::new();
    if file.keyboard != keyboard.name.as_ref() {
        warnings.push("Made for another keyboard; some keys may differ.".into());
    }
    if keyboard.capabilities.lightbar_leds == 0 && file.parts.lightbar.take().is_some() {
        warnings.push("Light bar settings are skipped.".into());
    }
    file.parts
        .check(&keyboard.capabilities)
        .map_err(|mismatch| {
            SharedString::from(format!(
                "`{}` does not fit this keyboard: {mismatch}",
                path.display()
            ))
        })?;
    Ok(Import {
        path,
        file,
        warnings,
    })
}

fn profile_name(text: &str) -> Result<Name, String> {
    if text.trim().is_empty() {
        return Err("Enter a name.".to_owned());
    }
    Name::try_from(text).map_err(|_| format!("Up to {NAME_LEN} bytes."))
}

fn copy_name(name: &str) -> Name {
    let mut copy = Name::new();
    for character in name.chars().chain(" copy".chars()) {
        if copy.push(character).is_err() {
            break;
        }
    }
    copy
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

fn hsla_of(color: Rgb) -> Hsla {
    Rgba {
        r: f32::from(color.red) / 255.0,
        g: f32::from(color.green) / 255.0,
        b: f32::from(color.blue) / 255.0,
        a: 1.0,
    }
    .into()
}

fn rgb_of(color: Hsla) -> Rgb {
    let rgba = color.to_rgb();
    Rgb {
        red: channel(rgba.r),
        green: channel(rgba.g),
        blue: channel(rgba.b),
    }
}

fn channel(value: f32) -> u8 {
    let scaled = (value.clamp(0.0, 1.0) * 255.0).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is clamped to 0..=255 and rounded"
    )]
    let byte = scaled as u8;
    byte
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
    fn a_copy_name_stays_within_the_limit() {
        assert_eq!(copy_name("Typing").as_str(), "Typing copy");
        let long = "y".repeat(30);
        assert_eq!(copy_name(&long).as_str(), format!("{long} c"));
        assert_eq!(copy_name(&"é".repeat(16)).len(), NAME_LEN);
    }

    #[test]
    fn file_stems_drop_path_characters() {
        assert_eq!(file_stem("FPS / Valorant"), "FPS _ Valorant");
        assert_eq!(file_stem("  "), "profile");
    }

    #[test]
    fn colours_survive_the_round_trip() {
        for color in [
            Rgb {
                red: 255,
                green: 40,
                blue: 40,
            },
            Rgb {
                red: 12,
                green: 200,
                blue: 99,
            },
            Rgb::default(),
        ] {
            assert_eq!(rgb_of(hsla_of(color)), color);
        }
    }

    #[test]
    fn steps_cover_every_request_of_a_preset() {
        assert_eq!(preset_steps(Preset::Custom), 2);
        assert_eq!(preset_steps(Preset::Fps), 7);
        assert_eq!(preset_steps(Preset::Racing), 5);
    }

    #[test]
    fn percentage_of_finished_steps() {
        assert!((percentage(1, 4) - 25.0).abs() < f32::EPSILON);
        assert!((percentage(9, 4) - 100.0).abs() < f32::EPSILON);
    }
}
