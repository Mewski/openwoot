use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::{Input, InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{lighting::Rgb, profile};

use super::{
    Name, ProfilesPage,
    editor::{BODY_TOP, banner},
    icon,
    preset::Preset,
    profile_name,
};
use crate::{
    color_picker::{ColorPicker, ColorPickerEvent},
    panel,
    store::Store,
};

const TEMPLATES_PER_ROW: usize = 3;
const ILLUSTRATION_HEIGHT: f32 = 132.0;
const TEMPLATE_LIST_WIDTH: f32 = 340.0;
const TEMPLATE_ROW_HEIGHT: f32 = 60.0;
const KEYCAP_SIZE: f32 = 34.0;

pub(super) struct Creator {
    page: WeakEntity<ProfilesPage>,
    store: Entity<Store>,
    library: bool,
    preset: Option<Preset>,
    step: Step,
    storage: Storage,
    name: Entity<InputState>,
    named: bool,
    picker: Entity<ColorPicker>,
    color: Rgb,
    colored: bool,
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
        let color = Preset::Custom.color();
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Profile name"));
        let picker = cx.new(|cx| ColorPicker::new(color, window, cx));
        let subscriptions = vec![
            cx.subscribe_in(
                &picker,
                window,
                |this, _, event: &ColorPickerEvent, _, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        this.color = *color;
                        this.colored = true;
                        cx.notify();
                    }
                },
            ),
            cx.subscribe_in(
                &name,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => {
                        this.named = true;
                        this.problem = None;
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => this.create(window, cx),
                    InputEvent::Focus | InputEvent::Blur => {}
                },
            ),
        ];
        Self {
            page,
            store,
            library,
            preset: None,
            step: Step::Templates,
            storage: Storage::Onboard,
            name,
            named: false,
            picker,
            color,
            colored: false,
            icon: None,
            problem: None,
            _subscriptions: subscriptions,
        }
    }

    fn store(&mut self, storage: Storage, cx: &mut Context<Self>) {
        if storage == Storage::Onboard || self.library {
            self.storage = storage;
            cx.notify();
        }
    }

    fn choose(&mut self, preset: Preset, window: &mut Window, cx: &mut Context<Self>) {
        self.preset = Some(preset);
        if !self.named {
            let name = default_name(preset);
            self.name
                .update(cx, |input, cx| input.set_value(name, window, cx));
        }
        if !self.colored {
            self.color = preset.color();
            let color = self.color;
            self.picker
                .update(cx, |picker, cx| picker.set_value(color, window, cx));
        }
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

    fn room(&self, cx: &App) -> Option<(usize, usize)> {
        let keyboard = self.store.read(cx).keyboard()?;
        Some((
            keyboard.profiles.len(),
            usize::from(keyboard.capabilities.profiles),
        ))
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(preset) = self.preset else {
            return;
        };
        if self
            .room(cx)
            .is_none_or(|(used, capacity)| used >= capacity)
        {
            return;
        }
        let text = self.name.read(cx).value();
        let text = if text.trim().is_empty() {
            SharedString::from(default_name(preset))
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
        let inactive = self.storage == Storage::Inactive;
        ColorPicker::remember(self.color, cx);
        self.page
            .update(cx, |page, cx| {
                page.create(preset, info, inactive, window, cx);
            })
            .ok();
    }

    fn templates(&self, cx: &Context<Self>) -> Div {
        let cards: Vec<AnyElement> = Preset::ALL
            .into_iter()
            .map(|preset| self.template_card(preset, cx).into_any_element())
            .collect();
        let mut rows = v_flex().w_full().gap_4();
        let mut cards = cards.into_iter().peekable();
        while cards.peek().is_some() {
            let mut row = h_flex().w_full().gap_4().items_start();
            for _ in 0..TEMPLATES_PER_ROW {
                row = row.child(div().flex_1().min_w_0().children(cards.next()));
            }
            rows = rows.child(row);
        }
        rows
    }

    fn template_card(&self, preset: Preset, cx: &Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let selected = self.preset == Some(preset);
        let hover = theme.secondary_hover;
        v_flex()
            .id(SharedString::from(format!("template-{}", preset.name())))
            .w_full()
            .rounded(theme.radius_lg)
            .overflow_hidden()
            .bg(theme.secondary)
            .border_1()
            .border_color(if selected {
                theme.primary
            } else {
                theme.secondary
            })
            .cursor_pointer()
            .when(!selected, |this| {
                this.hover(move |style| style.border_color(hover))
            })
            .on_click(cx.listener(move |this, _, window, cx| this.choose(preset, window, cx)))
            .child(
                div()
                    .w_full()
                    .h(px(ILLUSTRATION_HEIGHT))
                    .bg(theme.group_box)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(illustration(preset, cx)),
            )
            .child(
                v_flex()
                    .p_5()
                    .gap_1p5()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(SharedString::from(default_name(preset))),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(preset.description()),
                    ),
            )
    }

    fn details(&self, cx: &Context<Self>) -> Div {
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
        let room = self.room(cx);
        let full = room.is_none_or(|(used, capacity)| used >= capacity);
        let slots = room.map_or_else(SharedString::default, |(used, capacity)| {
            SharedString::from(format!("{used} / {capacity}"))
        });
        let types = v_flex()
            .gap_3()
            .child(self.storage_option(
                Storage::Onboard,
                "Onboard profile",
                "Stored on the keyboard and available on any computer.",
                Some(slots),
                cx,
            ))
            .child(self.storage_option(
                Storage::Inactive,
                "Inactive profile",
                "Stored on this computer only.",
                None,
                cx,
            ))
            .children(full.then(|| {
                div()
                    .text_sm()
                    .text_color(theme.danger)
                    .child("A new profile is built in a free keyboard slot; free one first.")
            }));
        let editor = v_flex()
            .flex_1()
            .min_w_0()
            .rounded(theme.radius_lg)
            .bg(theme.group_box)
            .child(banner(
                "new-profile-color",
                &self.picker,
                self.color,
                self.icon(),
                pick,
                cx,
            ))
            .child(
                v_flex()
                    .px_10()
                    .pt(px(BODY_TOP))
                    .pb_8()
                    .gap_6()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(panel::field("Profile name", Input::new(&self.name), cx))
                            .children(self.problem.clone().map(|problem| {
                                div().text_xs().text_color(theme.danger).child(problem)
                            })),
                    )
                    .child(panel::field("Profile type", types, cx)),
            );
        let list = v_flex()
            .flex_none()
            .w(px(TEMPLATE_LIST_WIDTH))
            .gap_2()
            .child(panel::label("Selected template", cx))
            .children(
                Preset::ALL
                    .into_iter()
                    .map(|preset| self.template_row(preset, cx)),
            );
        h_flex()
            .w_full()
            .items_start()
            .gap_5()
            .child(editor)
            .child(list)
    }

    fn storage_option(
        &self,
        storage: Storage,
        title: &'static str,
        description: &'static str,
        chip: Option<SharedString>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let selected = self.storage == storage;
        let available = storage == Storage::Onboard || self.library;
        let hover = theme.secondary_hover;
        h_flex()
            .id(title)
            .w_full()
            .p_4()
            .gap_3()
            .rounded(theme.radius)
            .bg(theme.secondary)
            .border_1()
            .border_color(if selected {
                theme.primary
            } else {
                theme.secondary
            })
            .when(!available, |this| this.opacity(0.5))
            .when(available && !selected, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.border_color(hover))
                    .on_click(cx.listener(move |this, _, _, cx| this.store(storage, cx)))
            })
            .child(
                v_flex()
                    .flex_1()
                    .gap_1()
                    .child(div().font_weight(FontWeight::MEDIUM).child(title))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(description),
                    ),
            )
            .children(chip.map(|chip| {
                div()
                    .px_1p5()
                    .rounded(px(4.0))
                    .bg(theme.group_box)
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(chip)
            }))
    }

    fn template_row(&self, preset: Preset, cx: &Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let selected = self.preset == Some(preset);
        let hover = theme.secondary_hover;
        h_flex()
            .id(SharedString::from(format!(
                "template-row-{}",
                preset.name()
            )))
            .w_full()
            .h(px(TEMPLATE_ROW_HEIGHT))
            .px_4()
            .gap_4()
            .rounded(theme.radius_lg)
            .bg(theme.secondary)
            .border_1()
            .border_color(if selected {
                theme.primary
            } else {
                theme.secondary
            })
            .cursor_pointer()
            .when(!selected, |this| {
                this.hover(move |style| style.border_color(hover))
            })
            .on_click(cx.listener(move |this, _, window, cx| this.choose(preset, window, cx)))
            .child(
                Icon::new(preset_icon(preset))
                    .small()
                    .text_color(if selected {
                        theme.primary
                    } else {
                        theme.muted_foreground
                    }),
            )
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(SharedString::from(default_name(preset))),
            )
    }
}

impl Render for Creator {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let full = self
            .room(cx)
            .is_none_or(|(used, capacity)| used >= capacity);
        let (subtitle, body, action) = if self.step == Step::Details {
            (
                "Name your profile and pick its colour.",
                self.details(cx),
                Button::new("create-profile")
                    .primary()
                    .label("Create profile")
                    .disabled(full)
                    .on_click(cx.listener(|this, _, window, cx| this.create(window, cx))),
            )
        } else {
            (
                "Select the type of profile you would like to create.",
                self.templates(cx),
                Button::new("continue-profile")
                    .primary()
                    .label("Continue")
                    .disabled(self.preset.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.proceed(cx))),
            )
        };
        v_flex()
            .w_full()
            .gap_5()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Creating new profile"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(subtitle),
                    ),
            )
            .child(body)
            .child(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel-profile")
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    )
                    .child(action),
            )
    }
}

fn default_name(preset: Preset) -> String {
    format!("{} profile", preset.name())
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
