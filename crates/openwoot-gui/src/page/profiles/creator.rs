use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, h_flex,
        input::{InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px, transparent_black,
};
use openwoot_protocol::{lighting::Rgb, profile};

use super::{
    Destination, LINKING_NEEDS_SERVICE, Name, ProfilesPage,
    apps::{AppList, AppListEvent},
    card::count_chip,
    editor::{BODY_PADDING_X, BODY_TOP, header, problem_text},
    icon,
    preset::Preset,
    profile_name, service_running,
};
use crate::{
    color_picker::{ColorPicker, ColorPickerEvent},
    panel::{self, ActionButton},
    store::Store,
    theme,
};

const TEMPLATES_PER_ROW: usize = 3;
const RADIUS: f32 = 8.0;
const INNER_RADIUS: f32 = 7.0;
const OPTION_RADIUS: f32 = 6.0;
const TITLE_TEXT: f32 = 24.0;
const TITLE_LINE: f32 = 28.8;
const BODY_TEXT: f32 = 16.0;
const NAME_LINE: f32 = 19.2;
const BODY_LINE: f32 = 22.4;
const INTRO_GAP: f32 = 8.0;
const VIEW_GAP: f32 = 24.0;
const GRID_GAP: f32 = 16.0;
const ILLUSTRATION_HEIGHT: f32 = 132.0;
const ILLUSTRATION_GAP: f32 = 8.0;
const TEMPLATE_PADDING: f32 = 24.0;
const TEMPLATE_TEXT_GAP: f32 = 8.0;
const DETAILS_GAP: f32 = 20.0;
const EDITOR_BOTTOM: f32 = 80.0;
const FIELDS_GAP: f32 = 32.0;
const FIELD_GAP: f32 = 8.0;
const OPTIONS_GAP: f32 = 12.0;
const OPTION_PADDING: f32 = 12.0;
const OPTION_GAP: f32 = 8.0;
const UNAVAILABLE_OPACITY: f32 = 0.5;
const LIST_WIDTH: f32 = 346.0;
const LIST_GAP: f32 = 8.0;
const ROW_HEIGHT: f32 = 68.0;
const ROW_PADDING_X: f32 = 20.0;
const ROW_GAP: f32 = 28.0;
const THUMB_WIDTH: f32 = 56.0;
const THUMB_HEIGHT: f32 = 34.0;
const THUMB_ICON: f32 = 20.0;
const BUTTON_GAP: f32 = 8.0;
const KEYCAP_SIZE: f32 = 34.0;
const NEW_COLOR: Rgb = Rgb {
    red: 255,
    green: 212,
    blue: 92,
};

pub(super) struct Creator {
    page: WeakEntity<ProfilesPage>,
    store: Entity<Store>,
    library: bool,
    preset: Option<Preset>,
    step: Step,
    storage: Storage,
    name: Entity<InputState>,
    picker: Entity<ColorPicker>,
    apps: Entity<AppList>,
    color: Rgb,
    icon: Option<u8>,
    problem: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Templates,
    Details,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Storage {
    Onboard,
    Linked,
    Inactive,
}

impl Creator {
    pub(super) fn new(
        page: WeakEntity<ProfilesPage>,
        store: Entity<Store>,
        library: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let color = NEW_COLOR;
        let name = cx.new(|cx| InputState::new(window, cx));
        let picker = cx.new(|cx| ColorPicker::new(color, window, cx));
        let linked = store.clone();
        let apps = cx.new(|cx| AppList::new(linked, None, Vec::new(), window, cx));
        let subscriptions = vec![
            cx.subscribe_in(
                &picker,
                window,
                |this, _, event: &ColorPickerEvent, _, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        this.color = *color;
                        cx.notify();
                    }
                },
            ),
            cx.subscribe_in(
                &name,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => {
                        this.problem = None;
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => this.create(window, cx),
                    InputEvent::Focus | InputEvent::Blur => {}
                },
            ),
            cx.subscribe(&apps, |_, _, _: &AppListEvent, cx| cx.notify()),
            cx.observe(&store, |_, _, cx| cx.notify()),
        ];
        Self {
            page,
            store,
            library,
            preset: None,
            step: Step::Templates,
            storage: Storage::Onboard,
            name,
            picker,
            apps,
            color,
            icon: None,
            problem: None,
            _subscriptions: subscriptions,
        }
    }

    fn store(&mut self, storage: Storage, cx: &mut Context<Self>) {
        if self.available(storage, cx) {
            self.storage = storage;
            cx.notify();
        }
    }

    fn available(&self, storage: Storage, cx: &App) -> bool {
        self.reason(storage, cx).is_none() && (storage != Storage::Inactive || self.library)
    }

    fn reason(&self, storage: Storage, cx: &App) -> Option<&'static str> {
        let (used, capacity) = self.room(storage, cx)?;
        match storage {
            Storage::Linked if used >= capacity => Some("Device has no free linked profile slots"),
            Storage::Linked if !service_running(cx) => Some(LINKING_NEEDS_SERVICE),
            Storage::Onboard | Storage::Inactive if used >= capacity => {
                Some("Device has no free onboard profile slots")
            }
            Storage::Onboard | Storage::Linked | Storage::Inactive => None,
        }
    }

    fn ready(&self, cx: &App) -> bool {
        self.preset.is_some()
            && self.available(self.storage, cx)
            && (self.storage != Storage::Linked || !self.apps.read(cx).links().is_empty())
    }

    fn choose(&mut self, preset: Preset, cx: &mut Context<Self>) {
        self.preset = Some(preset);
        cx.notify();
    }

    fn icon(&self) -> u8 {
        self.icon.unwrap_or_else(|| {
            self.preset
                .map_or(0, |preset| icon::index_of(preset_icon(preset)))
        })
    }

    fn proceed(&mut self, cx: &mut Context<Self>) {
        if self.preset.is_some() {
            self.step = Step::Details;
            cx.notify();
        }
    }

    fn cancel(&self, cx: &mut Context<Self>) {
        self.page.update(cx, ProfilesPage::close_creator).ok();
    }

    fn room(&self, storage: Storage, cx: &App) -> Option<(usize, usize)> {
        let keyboard = self.store.read(cx).keyboard()?;
        Some(match storage {
            Storage::Linked => (
                keyboard.linked.len(),
                usize::from(keyboard.capabilities.linked_profiles),
            ),
            Storage::Onboard | Storage::Inactive => (
                keyboard.profiles.len(),
                usize::from(keyboard.capabilities.profiles),
            ),
        })
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(preset) = self.preset else {
            return;
        };
        if !self.ready(cx) {
            return;
        }
        let text = self.name.read(cx).value();
        let text = if text.trim().is_empty() {
            SharedString::from(new_name(preset))
        } else {
            text
        };
        let name: Name = match profile_name(&text) {
            Ok(name) => name,
            Err(problem) => {
                self.problem = Some(problem.into());
                cx.notify();
                return;
            }
        };
        let info = profile::Info {
            name,
            color: self.color,
            icon: self.icon(),
        };
        let destination = match self.storage {
            Storage::Onboard => Destination::Onboard,
            Storage::Linked => Destination::Linked(self.apps.read(cx).links().to_vec()),
            Storage::Inactive => Destination::Inactive,
        };
        ColorPicker::remember(self.color, cx);
        self.page
            .update(cx, |page, cx| {
                page.create(preset, info, destination, window, cx);
            })
            .ok();
    }

    fn templates(&self, cx: &Context<Self>) -> Div {
        let mut rows = v_flex().w_full().gap(px(GRID_GAP));
        let mut cards = Preset::ALL
            .into_iter()
            .enumerate()
            .map(|(index, preset)| self.template_card(index, preset, cx))
            .peekable();
        while cards.peek().is_some() {
            let mut row = h_flex().w_full().items_stretch().gap(px(GRID_GAP));
            for _ in 0..TEMPLATES_PER_ROW {
                row = row.child(v_flex().flex_1().min_w_0().children(cards.next()));
            }
            rows = rows.child(row);
        }
        rows
    }

    fn template_card(&self, index: usize, preset: Preset, cx: &Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let selected = self.preset == Some(preset);
        let raised = theme::palette().overlay_hover;
        let border_hover = theme.secondary_hover;
        v_flex()
            .id(("template", index))
            .flex_1()
            .w_full()
            .rounded(px(RADIUS))
            .border_1()
            .border_color(if selected {
                theme.primary
            } else {
                transparent_black()
            })
            .bg(if selected { raised } else { theme.tab_active })
            .cursor_pointer()
            .when(!selected, |this| {
                this.hover(move |style| style.border_color(border_hover).bg(raised))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.choose(preset, cx)))
            .child(
                div()
                    .w_full()
                    .h(px(ILLUSTRATION_HEIGHT))
                    .rounded_t(px(INNER_RADIUS))
                    .bg(theme.group_box)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(illustration(preset, cx)),
            )
            .child(
                v_flex()
                    .mt(px(ILLUSTRATION_GAP))
                    .p(px(TEMPLATE_PADDING))
                    .gap(px(TEMPLATE_TEXT_GAP))
                    .child(
                        div()
                            .text_size(px(BODY_TEXT))
                            .line_height(px(NAME_LINE))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child(preset.title()),
                    )
                    .child(
                        div()
                            .text_size(px(BODY_TEXT))
                            .line_height(px(BODY_LINE))
                            .text_color(theme.group_box_foreground)
                            .child(preset.description()),
                    ),
            )
    }

    fn types(&self, cx: &Context<Self>) -> Div {
        let supports_linked = self
            .store
            .read(cx)
            .keyboard()
            .is_some_and(|keyboard| keyboard.capabilities.linked_profiles > 0);
        let reason = self.reason(self.storage, cx).or_else(|| {
            [Storage::Onboard, Storage::Linked]
                .into_iter()
                .filter(|storage| *storage != Storage::Linked || supports_linked)
                .find_map(|storage| self.reason(storage, cx))
        });
        v_flex()
            .gap(px(OPTIONS_GAP))
            .child(panel::label("Profile Type", cx))
            .child(self.storage_option(
                Storage::Onboard,
                "Onboard profile",
                "Stored on your device and can be accessed anytime.",
                self.room(Storage::Onboard, cx),
                cx,
            ))
            .when(supports_linked, |this| {
                this.child(self.storage_option(
                    Storage::Linked,
                    "Linked Profile",
                    "Switches on when the linked app is in front.",
                    self.room(Storage::Linked, cx),
                    cx,
                ))
            })
            .child(self.storage_option(
                Storage::Inactive,
                "Inactive profile",
                "Stored in openwoot and accessible only while it is running.",
                None,
                cx,
            ))
            .children(reason.map(|reason| problem_text(reason.into(), cx)))
            .when(self.storage == Storage::Linked, |this| {
                this.child(self.apps.clone())
            })
    }

    fn details(&self, window: &Window, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let creator = cx.entity().downgrade();
        let pick = move |icon: u8, _: &mut Window, cx: &mut App| {
            creator
                .update(cx, |creator, cx| {
                    creator.icon = Some(icon);
                    cx.notify();
                })
                .ok();
        };
        let types = self.types(cx);
        let editor = v_flex()
            .flex_1()
            .min_w_0()
            .pb(px(EDITOR_BOTTOM))
            .rounded(px(RADIUS))
            .bg(theme.group_box)
            .child(header(
                "new-profile",
                &self.picker,
                self.color,
                self.icon(),
                pick,
            ))
            .child(
                v_flex()
                    .px(px(BODY_PADDING_X))
                    .pt(px(BODY_TOP))
                    .gap(px(FIELDS_GAP))
                    .child(
                        v_flex()
                            .gap(px(FIELD_GAP))
                            .child(panel::field(
                                "Profile Name",
                                panel::text_field(
                                    &self.name,
                                    self.preset.map_or_else(String::new, new_name),
                                    window,
                                    cx,
                                ),
                                cx,
                            ))
                            .children(
                                self.problem
                                    .clone()
                                    .map(|problem| problem_text(problem, cx)),
                            ),
                    )
                    .child(types),
            );
        let list = v_flex()
            .flex_none()
            .w(px(LIST_WIDTH))
            .gap(px(LIST_GAP))
            .child(panel::label("Selected Template", cx))
            .children(
                Preset::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, preset)| self.template_row(index, preset, cx)),
            );
        h_flex()
            .w_full()
            .items_start()
            .gap(px(DETAILS_GAP))
            .child(editor)
            .child(list)
    }

    fn storage_option(
        &self,
        storage: Storage,
        title: &'static str,
        description: &'static str,
        room: Option<(usize, usize)>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let selected = self.storage == storage;
        let available = self.available(storage, cx);
        let (border_hover, hover) = (theme.secondary_hover, theme.secondary);
        v_flex()
            .id(title)
            .w_full()
            .p(px(OPTION_PADDING))
            .gap(px(OPTION_GAP))
            .rounded(px(OPTION_RADIUS))
            .border_1()
            .border_color(if selected {
                theme.primary
            } else {
                theme.border
            })
            .bg(if selected {
                theme.secondary
            } else {
                theme.accent
            })
            .when(!available, |this| this.opacity(UNAVAILABLE_OPACITY))
            .when(available && !selected, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.border_color(border_hover).bg(hover))
                    .on_click(cx.listener(move |this, _, _, cx| this.store(storage, cx)))
            })
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap(px(OPTION_GAP))
                    .child(
                        div()
                            .text_size(px(BODY_TEXT))
                            .line_height(px(NAME_LINE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child(title),
                    )
                    .children(room.map(|(used, capacity)| count_chip(used, capacity, cx))),
            )
            .child(
                div()
                    .text_size(px(BODY_TEXT))
                    .line_height(px(BODY_LINE))
                    .text_color(theme.group_box_foreground)
                    .child(description),
            )
    }

    fn template_row(&self, index: usize, preset: Preset, cx: &Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let selected = self.preset == Some(preset);
        let (border_hover, hover) = (theme.secondary_hover, theme.tab_active);
        h_flex()
            .id(("template-row", index))
            .w_full()
            .h(px(ROW_HEIGHT))
            .px(px(ROW_PADDING_X))
            .gap(px(ROW_GAP))
            .rounded(px(RADIUS))
            .border_1()
            .border_color(if selected {
                theme.primary
            } else {
                theme.border
            })
            .bg(if selected {
                theme::palette().overlay_hover
            } else {
                theme.group_box
            })
            .when(!selected, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.border_color(border_hover).bg(hover))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.choose(preset, cx)))
            .child(
                div()
                    .flex_none()
                    .w(px(THUMB_WIDTH))
                    .h(px(THUMB_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(preset_icon(preset))
                            .size(px(THUMB_ICON))
                            .text_color(if selected {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            }),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(BODY_TEXT))
                    .line_height(px(NAME_LINE))
                    .font_weight(if selected {
                        FontWeight::BOLD
                    } else {
                        FontWeight::SEMIBOLD
                    })
                    .text_color(theme.foreground)
                    .child(preset.title()),
            )
    }
}

impl Render for Creator {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ready = self.ready(cx);
        let (subtitle, body, action) = if self.step == Step::Details {
            (
                "Name your profile, customize its appearance, and select where it is stored.",
                self.details(window, cx),
                ActionButton::new("create-profile", "Create profile")
                    .large()
                    .primary()
                    .disabled(!ready)
                    .on_click(cx.listener(|this, _, window, cx| this.create(window, cx))),
            )
        } else {
            (
                "Select the type of profile you would like to create",
                self.templates(cx),
                ActionButton::new("continue-profile", "Continue")
                    .large()
                    .primary()
                    .disabled(self.preset.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.proceed(cx))),
            )
        };
        let intro = v_flex()
            .gap(px(INTRO_GAP))
            .child(
                div()
                    .text_size(px(TITLE_TEXT))
                    .line_height(px(TITLE_LINE))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.foreground)
                    .child("Creating new profile"),
            )
            .child(
                div()
                    .text_size(px(BODY_TEXT))
                    .line_height(px(NAME_LINE))
                    .text_color(theme.group_box_foreground)
                    .child(subtitle),
            );
        panel::page_with_heading(
            "profile-creator",
            intro,
            v_flex().w_full().gap(px(VIEW_GAP)).child(body).child(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap(px(BUTTON_GAP))
                    .child(
                        ActionButton::new("cancel-profile", "Cancel")
                            .large()
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    )
                    .child(action),
            ),
        )
        .gap(px(VIEW_GAP))
    }
}

fn new_name(preset: Preset) -> String {
    format!("New {}", preset.title())
}

const fn preset_icon(preset: Preset) -> Catalog {
    match preset {
        Preset::Fps => Catalog::Crosshair,
        Preset::AnalogMovement => Catalog::Joystick,
        Preset::Racing => Catalog::Gauge,
        Preset::Typing => Catalog::Keyboard,
        Preset::Custom => Catalog::Plus,
    }
}

fn illustration(preset: Preset, cx: &App) -> AnyElement {
    let theme = cx.theme();
    match preset {
        Preset::Fps => Icon::new(Catalog::Crosshair)
            .size(px(64.0))
            .text_color(theme.primary)
            .into_any_element(),
        Preset::AnalogMovement => keycaps(["W", "A", "S", "D"], [true, true, false, false], cx),
        Preset::Racing => keycaps(["RT", "L", "LT", "L"], [true, false, true, false], cx),
        Preset::Typing => div()
            .text_size(px(48.0))
            .font_weight(FontWeight::BOLD)
            .text_color(theme.primary)
            .child("Abc")
            .into_any_element(),
        Preset::Custom => div()
            .w(px(180.0))
            .h(px(64.0))
            .rounded(theme.radius_lg)
            .border_2()
            .border_color(theme.primary)
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(36.0))
            .font_weight(FontWeight::BOLD)
            .text_color(theme.primary)
            .child("?")
            .into_any_element(),
    }
}

fn keycaps(labels: [&'static str; 4], lit: [bool; 4], cx: &App) -> AnyElement {
    let theme = cx.theme();
    let cap = |index: usize| {
        let on = lit.get(index).copied().unwrap_or(false);
        div()
            .size(px(KEYCAP_SIZE))
            .rounded(px(6.0))
            .border_1()
            .border_color(if on { theme.primary } else { theme.border })
            .bg(if on { theme.primary } else { theme.secondary })
            .text_color(if on {
                theme.primary_foreground
            } else {
                theme.foreground
            })
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .flex()
            .items_center()
            .justify_center()
            .children(labels.get(index).copied())
    };
    v_flex()
        .items_center()
        .gap_1()
        .child(cap(0))
        .child(h_flex().gap_1().child(cap(1)).child(cap(2)).child(cap(3)))
        .into_any_element()
}
