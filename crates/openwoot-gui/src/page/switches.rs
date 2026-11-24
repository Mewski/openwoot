mod catalog;

use std::{
    collections::{BTreeMap, BTreeSet},
    mem,
};

use gpui_kit::{
    App, AppContext as _, Context, Div, Entity, Hsla, InteractiveElement as _, IntoElement,
    Modifiers, ParentElement as _, Pixels, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
    component::{
        ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _,
        alert::Alert,
        button::{Button, ButtonGroup, ButtonVariants as _},
        h_flex,
        select::{SearchableVec, Select, SelectEvent, SelectGroup, SelectItem, SelectState},
        slider::{Slider, SliderEvent, SliderState},
        v_flex,
    },
    div, hsla,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    configuration::{Capabilities, Validate as _},
    endpoints::SetSwitches,
    switches::{self, MAX_TUNED, Switch},
};

use crate::{
    keyboard::{KeyFace, KeyboardView, Selection},
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    usage,
};

const MILLIMETRES_PER_STEP: f32 = 0.05;
const KEYBOARD_MARGIN: f32 = 340.0;
const SLIDER_WIDTH: f32 = 240.0;
const SELECT_WIDTH: f32 = 280.0;
const MENU_HEIGHT: f32 = 360.0;
const SWATCH_SIZE: f32 = 10.0;
const INHERITED_OPACITY: f32 = 0.4;
const HUE_STEP: f32 = 0.618_034;
const MODEL_SATURATION: f32 = 0.45;
const MODEL_LIGHTNESS: f32 = 0.38;
const FACTORY_MODEL: u8 = 0;
const EDITION_FACTORY_MODEL: u8 = 1;
const FIRST_BRAND: &str = "Wooting";
const OTHER_BRAND: &str = "Other";

type ModelChoices = SearchableVec<SelectGroup<ModelChoice>>;

pub(crate) struct SwitchesPage {
    store: Entity<Store>,
    selection: Selection,
    labels: KeyLabels,
    model_select: Entity<SelectState<ModelChoices>>,
    spacer: Entity<SliderState>,
    shown: Option<Shown>,
    problem: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyLabels {
    Switch,
    Travel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
    selection: Selection,
    switches: Option<switches::Settings>,
    models: switches::Models,
    available: u8,
}

#[derive(Debug, Clone)]
struct ModelChoice {
    model: u8,
    title: SharedString,
    name: SharedString,
    variants: Option<SharedString>,
    travel: SharedString,
}

struct Controls {
    model: Option<u8>,
    spacer: f32,
    spacer_max: f32,
}

impl SwitchesPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let model_select = cx.new(|cx| {
            SelectState::new(ModelChoices::new(Vec::new()), None, window, cx).searchable(true)
        });
        let spacer = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(MILLIMETRES_PER_STEP)
                .step(MILLIMETRES_PER_STEP)
                .default_value(0.0)
        });
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected => {
                    this.selection.clear();
                    this.problem = None;
                    this.shown = None;
                    this.sync_controls(window, cx);
                }
                StoreEvent::Disconnected
                | StoreEvent::ProfileChanged
                | StoreEvent::Reloaded
                | StoreEvent::Failed(_) => {}
            }),
            cx.observe_in(&store, window, |this, _, window, cx| {
                this.sync_controls(window, cx);
            }),
            cx.subscribe_in(
                &model_select,
                window,
                |this, _, event: &SelectEvent<ModelChoices>, window, cx| {
                    if let SelectEvent::Confirm(Some(model)) = event {
                        this.assign_model(*model, window, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &spacer,
                window,
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(_) => cx.notify(),
                    SliderEvent::Release(value) => this.set_spacer(value.end(), window, cx),
                },
            ),
        ];
        let mut page = Self {
            store,
            selection: Selection::default(),
            labels: KeyLabels::Switch,
            model_select,
            spacer,
            shown: None,
            problem: None,
            _subscriptions: subscriptions,
        };
        page.sync_controls(window, cx);
        page
    }

    fn sync_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (shown, choices, controls) = {
            let Some(keyboard) = self.store.read(cx).keyboard() else {
                return;
            };
            let shown = Shown {
                selection: self.selection.clone(),
                switches: keyboard.switches.clone(),
                models: keyboard.models.clone(),
                available: keyboard.capabilities.switch_models,
            };
            if self.shown.as_ref() == Some(&shown) {
                return;
            }
            let rebuild = self
                .shown
                .as_ref()
                .is_none_or(|old| old.models != shown.models || old.available != shown.available);
            (
                shown,
                rebuild.then(|| choices(keyboard)),
                Controls::of(keyboard, &self.selection),
            )
        };
        if let Some(choices) = choices {
            self.model_select
                .update(cx, |select, cx| select.set_items(choices, window, cx));
        }
        self.model_select
            .update(cx, |select, cx| match controls.model {
                Some(model) => select.set_selected_value(&model, window, cx),
                None => select.set_selected_index(None, window, cx),
            });
        let spacer_max = controls.spacer_max.max(MILLIMETRES_PER_STEP);
        self.spacer.update(cx, |slider, cx| {
            let current = mem::replace(slider, SliderState::new());
            *slider = current.min(0.0).max(spacer_max);
            slider.set_value(controls.spacer.clamp(0.0, spacer_max), window, cx);
        });
        self.shown = Some(shown);
        cx.notify();
    }

    fn click_key(
        &mut self,
        key: usize,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection.click(key, modifiers);
        self.sync_controls(window, cx);
    }

    fn select_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(keyboard) = self.store.read(cx).keyboard() {
            self.selection.select_all(keyboard);
        }
        self.sync_controls(window, cx);
    }

    fn select_model(&mut self, model: u8, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(keyboard) = self.store.read(cx).keyboard() {
            self.selection.set(
                (0..keyboard.key_count()).filter(|key| {
                    keyboard.has_key(*key) && effective(keyboard, *key).model == model
                }),
            );
        }
        self.sync_controls(window, cx);
    }

    fn clear_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selection.clear();
        self.sync_controls(window, cx);
    }

    fn show_labels(&mut self, labels: KeyLabels, cx: &mut Context<Self>) {
        self.labels = labels;
        cx.notify();
    }

    fn change(
        &mut self,
        edit: impl FnOnce(&mut switches::Settings, &Selection, Switch, &[switches::Model]),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let checked = {
            let Some(keyboard) = self.store.read(cx).keyboard() else {
                return;
            };
            let Some(current) = &keyboard.switches else {
                return;
            };
            let mut settings = current.clone();
            edit(
                &mut settings,
                &self.selection,
                factory_switch(&keyboard.capabilities),
                &keyboard.models,
            );
            if settings == *current {
                return;
            }
            settings
                .validate(&keyboard.capabilities)
                .and_then(|()| settings.check_models(&keyboard.models))
                .map(|()| settings)
        };
        match checked {
            Err(invalid) => {
                self.problem = Some(format!("could not change the switches: {invalid}").into());
                self.shown = None;
                self.sync_controls(window, cx);
            }
            Ok(settings) => {
                self.problem = None;
                let applied = self.store.update(cx, |store, cx| {
                    store.apply::<SetSwitches>(settings, Scope::Keyboard, cx)
                });
                cx.spawn_in(window, async move |this, cx| {
                    let applied = applied.await.is_ok();
                    this.update_in(cx, |page, window, cx| {
                        if applied {
                            page.store
                                .update(cx, |store, cx| store.reload_switches(cx))
                                .detach();
                        } else {
                            page.shown = None;
                            page.sync_controls(window, cx);
                        }
                    })
                    .ok();
                })
                .detach();
            }
        }
        cx.notify();
    }

    fn assign_model(&mut self, model: u8, window: &mut Window, cx: &mut Context<Self>) {
        self.change(
            |settings, selection, factory, models| {
                let travel = models
                    .get(usize::from(model))
                    .map_or(0, |entry| entry.travel);
                if selection.is_empty() {
                    let offset = settings.default.unwrap_or(factory).travel_offset;
                    settings.default = Some(fitted(model, travel, offset));
                } else {
                    for key in selection.iter() {
                        let offset = settings.of_key(key).unwrap_or(factory).travel_offset;
                        if let Some(slot) = settings.keys.get_mut(key) {
                            *slot = Some(fitted(model, travel, offset));
                        }
                    }
                }
            },
            window,
            cx,
        );
    }

    fn use_default_switch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change(
            |settings, selection, _, _| {
                for key in selection.iter() {
                    if let Some(slot) = settings.keys.get_mut(key) {
                        *slot = None;
                    }
                }
            },
            window,
            cx,
        );
    }

    fn use_factory_switch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change(|settings, _, _, _| settings.default = None, window, cx);
    }

    fn set_spacer(&mut self, millimetres: f32, window: &mut Window, cx: &mut Context<Self>) {
        let offset = steps_of(millimetres);
        self.change(
            |settings, selection, factory, _| {
                if selection.is_empty() {
                    let current = settings.default.unwrap_or(factory);
                    if current.travel_offset != offset {
                        settings.default = Some(Switch {
                            model: current.model,
                            travel_offset: offset,
                        });
                    }
                } else {
                    for key in selection.iter() {
                        let current = settings.of_key(key).unwrap_or(factory);
                        if current.travel_offset != offset
                            && let Some(slot) = settings.keys.get_mut(key)
                        {
                            *slot = Some(Switch {
                                model: current.model,
                                travel_offset: offset,
                            });
                        }
                    }
                }
            },
            window,
            cx,
        );
    }

    fn keyboard_section(&self, keyboard: &Keyboard, width: Pixels, cx: &Context<Self>) -> Div {
        let labels = self.labels;
        let page = cx.weak_entity();
        let view = KeyboardView::new("switch-keys", keyboard, |key| face(keyboard, labels, key))
            .selection(&self.selection)
            .fit_width(width)
            .on_key_click(move |key, event, window, cx| {
                page.update(cx, |page, cx| {
                    page.click_key(key, event.modifiers(), window, cx);
                })
                .ok();
            });
        let description: SharedString = if self.selection.is_empty() {
            "Select keys on the keyboard (Shift-, ⌘- or Ctrl-click adds and removes keys), then \
             choose the switch installed in them. With no keys selected, the choice sets the \
             default switch."
                .into()
        } else {
            format!(
                "{} selected: the choices below apply to them only.",
                keys_text(self.selection.len())
            )
            .into()
        };
        panel::section("Keyboard", Some(description), cx)
            .child(self.toolbar(cx))
            .child(view)
            .child(models_in_use(keyboard, cx))
    }

    fn toolbar(&self, cx: &Context<Self>) -> Div {
        h_flex()
            .w_full()
            .gap_2()
            .flex_wrap()
            .justify_between()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("select-all")
                            .small()
                            .outline()
                            .label("Select all")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.select_all(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("clear-selection")
                            .small()
                            .ghost()
                            .label("Clear selection")
                            .disabled(self.selection.is_empty())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.clear_selection(window, cx);
                            })),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Show on keys"),
                    )
                    .child(
                        ButtonGroup::new("switch-labels")
                            .small()
                            .outline()
                            .child(
                                Button::new("labels-switch")
                                    .label("Switch")
                                    .selected(self.labels == KeyLabels::Switch),
                            )
                            .child(
                                Button::new("labels-travel")
                                    .label("Travel")
                                    .selected(self.labels == KeyLabels::Travel),
                            )
                            .on_click(cx.listener(|this, clicks: &Vec<usize>, _, cx| {
                                let labels = if clicks.contains(&1) {
                                    KeyLabels::Travel
                                } else {
                                    KeyLabels::Switch
                                };
                                this.show_labels(labels, cx);
                            })),
                    ),
            )
    }

    fn switch_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let targets = targets(keyboard, &self.selection);
        let model = same(targets.iter().map(|switch| switch.model));
        let travel = model
            .and_then(|model| keyboard.models.get(usize::from(model)))
            .map_or_else(
                || "Mixed".to_owned(),
                |entry| format!("{:.2} mm", millimetres(entry.travel)),
            );
        let (title, description): (SharedString, SharedString) = if self.selection.is_empty() {
            (
                "Default switch".into(),
                "Keys without their own switch use the default switch.".into(),
            )
        } else {
            (
                format!("Switch of {}", keys_text(self.selection.len())).into(),
                "Choose the switch installed in the selected keys.".into(),
            )
        };
        let reset = if self.selection.is_empty() {
            let factory = factory_switch(&keyboard.capabilities);
            let has_default = keyboard
                .switches
                .as_ref()
                .is_some_and(|switches| switches.default.is_some());
            panel::row(
                "Use the factory switch",
                Some(
                    format!(
                        "Without a default, keys use the switch the keyboard was built with ({}).",
                        product_name(factory.model)
                    )
                    .into(),
                ),
                Button::new("use-factory-switch")
                    .small()
                    .outline()
                    .label("Use factory switch")
                    .disabled(!has_default)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.use_factory_switch(window, cx)),
                    ),
                cx,
            )
        } else {
            let has_own = keyboard.switches.as_ref().is_some_and(|switches| {
                self.selection
                    .iter()
                    .any(|key| switches.keys.get(key).copied().flatten().is_some())
            });
            panel::row(
                "Use the default switch",
                Some("The selected keys follow the default switch again.".into()),
                Button::new("use-default-switch")
                    .small()
                    .outline()
                    .label("Use default switch")
                    .disabled(!has_own)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.use_default_switch(window, cx)),
                    ),
                cx,
            )
        };
        panel::section(title, Some(description), cx)
            .child(panel::row(
                "Switch",
                None,
                Select::new(&self.model_select)
                    .w(px(SELECT_WIDTH))
                    .placeholder("Mixed switches")
                    .search_placeholder("Search for a switch")
                    .menu_max_h(px(MENU_HEIGHT)),
                cx,
            ))
            .child(panel::row(
                "Total travel",
                Some("The travel of the switch without spacers.".into()),
                div().text_sm().child(travel),
                cx,
            ))
            .child(reset)
            .child(panel::note(
                "Don't see your switch on the list? Select a supported switch from the same \
                 brand, or one with the same total travel distance.",
                cx,
            ))
    }

    fn spacer_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let size = self.spacer.read(cx).value().end();
        let targets = targets(keyboard, &self.selection);
        let label = if same(targets.iter().map(|switch| switch.travel_offset)).is_some() {
            format!("Size: {size:.2} mm")
        } else {
            "Size: mixed".to_owned()
        };
        let tuned = keyboard.switches.as_ref().map_or(0, tuned_pairs);
        panel::section(
            "Spacers",
            Some(
                "If you have spacers installed, select those keys and enter the exact size. With \
                 no keys selected, the size applies to the default switch."
                    .into(),
            ),
            cx,
        )
        .child(panel::row(
            label,
            Some(
                "Spacers shorten the travel in steps of 0.05 mm; the size stays below the travel \
                 of the switch."
                    .into(),
            ),
            div().w(px(SLIDER_WIDTH)).child(Slider::new(&self.spacer)),
            cx,
        ))
        .child(panel::row(
            "Spacer combinations",
            Some(
                format!(
                    "Every different pair of switch and spacer size counts once; the keyboard \
                     holds at most {MAX_TUNED}."
                )
                .into(),
            ),
            div().text_sm().child(format!("{tuned} of {MAX_TUNED}")),
            cx,
        ))
        .when_some(self.problem.clone(), |this, problem| {
            this.child(Alert::error("switch-problem", problem))
        })
    }
}

impl Render for SwitchesPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = panel::page(
            "Switches",
            "Assign the switches installed on your keyboard. This applies a calibrated analog \
             curve and total travel distance to each switch, allowing the keyboard to detect the \
             precise position of every key.",
            cx,
        );
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return page;
        };
        if keyboard.switches.is_none() {
            return page.child(panel::note(
                "The keyboard does not report its switches, so they cannot be changed here.",
                cx,
            ));
        }
        let width = window.viewport_size().width - px(KEYBOARD_MARGIN);
        page.child(self.keyboard_section(keyboard, width, cx))
            .child(
                h_flex()
                    .w_full()
                    .gap_4()
                    .items_start()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(self.switch_section(keyboard, cx)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(self.spacer_section(keyboard, cx)),
                    ),
            )
    }
}

impl ModelChoice {
    fn new(model: u8, entry: switches::Model) -> Self {
        let product = catalog::product(model);
        Self {
            model,
            title: product_name(model),
            name: product.map_or_else(|| unknown_name(model).into(), |product| product.name.into()),
            variants: product
                .filter(|product| !product.variants.is_empty())
                .map(|product| product.variants.join(", ").into()),
            travel: format!("{:.2} mm", millimetres(entry.travel)).into(),
        }
    }
}

impl SelectItem for ModelChoice {
    type Value = u8;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        h_flex()
            .w_full()
            .gap_3()
            .justify_between()
            .child(
                v_flex()
                    .min_w_0()
                    .child(self.name.clone())
                    .when_some(self.variants.clone(), |this, variants| {
                        this.child(div().text_xs().text_color(muted).child(variants))
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .text_xs()
                    .text_color(muted)
                    .child(self.travel.clone()),
            )
    }

    fn value(&self) -> &Self::Value {
        &self.model
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.title.to_lowercase().contains(&query)
            || self
                .variants
                .as_ref()
                .is_some_and(|variants| variants.to_lowercase().contains(&query))
    }
}

impl Controls {
    fn of(keyboard: &Keyboard, selection: &Selection) -> Self {
        let targets = targets(keyboard, selection);
        let shortest = targets
            .iter()
            .filter_map(|switch| keyboard.models.get(usize::from(switch.model)))
            .map(|model| model.travel)
            .min()
            .unwrap_or(0);
        Self {
            model: same(targets.iter().map(|switch| switch.model)),
            spacer: targets
                .first()
                .map_or(0.0, |switch| millimetres(switch.travel_offset)),
            spacer_max: millimetres(shortest.saturating_sub(1)),
        }
    }
}

fn choices(keyboard: &Keyboard) -> ModelChoices {
    let mut groups: Vec<SelectGroup<ModelChoice>> = Vec::new();
    let available = usize::from(keyboard.capabilities.switch_models);
    for (model, entry) in (0..=u8::MAX).zip(&keyboard.models).take(available) {
        let choice = ModelChoice::new(model, *entry);
        let brand = catalog::product(model).map_or(OTHER_BRAND, |product| product.brand);
        if let Some(group) = groups.iter_mut().find(|group| &*group.title == brand) {
            group.items.push(choice);
        } else {
            groups.push(SelectGroup::new(brand).item(choice));
        }
    }
    groups.sort_by_cached_key(|group| (&*group.title != FIRST_BRAND, group.title.to_lowercase()));
    for group in &mut groups {
        group
            .items
            .sort_by(|first, second| first.name.cmp(&second.name));
    }
    ModelChoices::new(groups)
}

fn models_in_use(keyboard: &Keyboard, cx: &Context<SwitchesPage>) -> Div {
    let mut counts = BTreeMap::<u8, usize>::new();
    for key in (0..keyboard.key_count()).filter(|key| keyboard.has_key(*key)) {
        *counts.entry(effective(keyboard, key).model).or_default() += 1;
    }
    let muted = cx.theme().muted_foreground;
    v_flex()
        .gap_1()
        .children(counts.into_iter().map(|(model, count)| {
            h_flex()
                .id(SharedString::from(format!("switch-model-{model}")))
                .gap_2()
                .items_center()
                .text_xs()
                .cursor_pointer()
                .child(
                    div()
                        .size(px(SWATCH_SIZE))
                        .rounded(px(3.0))
                        .bg(model_color(model)),
                )
                .child(product_name(model))
                .child(div().text_color(muted).child(keys_text(count)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.select_model(model, window, cx);
                }))
        }))
        .child(div().text_xs().text_color(muted).child(
            "Keys with their own switch are drawn solid, keys that use the default switch faded. \
             Click a switch to select its keys.",
        ))
}

fn face(keyboard: &Keyboard, labels: KeyLabels, key: usize) -> KeyFace {
    let legend = keyboard
        .legends
        .get(key)
        .map_or_default(|action| SharedString::from(usage::legend(*action)));
    let Some(switches) = &keyboard.switches else {
        return KeyFace {
            legend,
            ..KeyFace::default()
        };
    };
    let switch = effective(keyboard, key);
    let own = switches.keys.get(key).copied().flatten().is_some();
    let detail = match labels {
        KeyLabels::Switch => short_name(switch.model),
        KeyLabels::Travel => keyboard.models.get(usize::from(switch.model)).map_or_else(
            || "?".into(),
            |model| {
                format!(
                    "{:.2}",
                    millimetres(model.travel.saturating_sub(switch.travel_offset))
                )
                .into()
            },
        ),
    };
    let color = model_color(switch.model);
    KeyFace {
        legend,
        detail: Some(detail),
        fill: Some(if own {
            color
        } else {
            color.opacity(INHERITED_OPACITY)
        }),
        travel: None,
        dimmed: false,
    }
}

fn targets(keyboard: &Keyboard, selection: &Selection) -> Vec<Switch> {
    let Some(switches) = &keyboard.switches else {
        return Vec::new();
    };
    let factory = factory_switch(&keyboard.capabilities);
    if selection.is_empty() {
        vec![switches.default.unwrap_or(factory)]
    } else {
        selection
            .iter()
            .map(|key| switches.of_key(key).unwrap_or(factory))
            .collect()
    }
}

fn effective(keyboard: &Keyboard, key: usize) -> Switch {
    let factory = factory_switch(&keyboard.capabilities);
    keyboard
        .switches
        .as_ref()
        .and_then(|switches| switches.of_key(key))
        .unwrap_or(factory)
}

const fn factory_switch(capabilities: &Capabilities) -> Switch {
    Switch {
        model: if capabilities.edition.is_some() {
            EDITION_FACTORY_MODEL
        } else {
            FACTORY_MODEL
        },
        travel_offset: 0,
    }
}

const fn fitted(model: u8, travel: u8, offset: u8) -> Switch {
    Switch {
        model,
        travel_offset: if offset < travel { offset } else { 0 },
    }
}

fn tuned_pairs(switches: &switches::Settings) -> usize {
    switches
        .switches()
        .filter(|switch| switch.is_tuned())
        .map(|switch| (switch.model, switch.travel_offset))
        .collect::<BTreeSet<_>>()
        .len()
}

fn same<T: PartialEq>(values: impl IntoIterator<Item = T>) -> Option<T> {
    let mut values = values.into_iter();
    let first = values.next()?;
    values.all(|value| value == first).then_some(first)
}

fn product_name(model: u8) -> SharedString {
    catalog::product(model).map_or_else(
        || unknown_name(model).into(),
        |product| format!("{} {}", product.brand, product.name).into(),
    )
}

fn short_name(model: u8) -> SharedString {
    catalog::product(model).map_or_else(
        || format!("#{model}").into(),
        |product| product.short.into(),
    )
}

fn unknown_name(model: u8) -> String {
    format!("Switch model {model}")
}

fn model_color(model: u8) -> Hsla {
    hsla(
        (f32::from(model) * HUE_STEP).fract(),
        MODEL_SATURATION,
        MODEL_LIGHTNESS,
        1.0,
    )
}

fn millimetres(steps: u8) -> f32 {
    f32::from(steps) * MILLIMETRES_PER_STEP
}

fn steps_of(millimetres: f32) -> u8 {
    let steps = (millimetres / MILLIMETRES_PER_STEP)
        .round()
        .clamp(0.0, f32::from(u8::MAX));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to 0..=255"
    )]
    let steps = steps as u8;
    steps
}

fn keys_text(count: usize) -> String {
    if count == 1 {
        "1 key".to_owned()
    } else {
        format!("{count} keys")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn switch(model: u8, travel_offset: u8) -> Switch {
        Switch {
            model,
            travel_offset,
        }
    }

    fn capabilities(edition: Option<u8>) -> Capabilities {
        Capabilities {
            key_rows: 1,
            key_columns: 4,
            profiles: 1,
            layouts: std::iter::empty().collect(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 49,
            edition,
        }
    }

    #[test]
    fn spacer_sizes_round_to_steps_of_five_hundredths_of_a_millimetre() {
        assert_eq!(steps_of(0.5), 10);
        assert_eq!(steps_of(0.524), 10);
        assert_eq!(steps_of(0.53), 11);
        assert_eq!(steps_of(-1.0), 0);
        assert_eq!(steps_of(100.0), u8::MAX);
        assert!((millimetres(10) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn each_tuned_pair_of_switch_and_spacer_counts_once() {
        let settings = switches::Settings {
            default: Some(switch(0, 10)),
            keys: [
                Some(switch(0, 10)),
                Some(switch(2, 10)),
                None,
                Some(switch(2, 0)),
            ]
            .into_iter()
            .collect(),
        };
        assert_eq!(tuned_pairs(&settings), 2);
    }

    #[test]
    fn a_new_switch_keeps_a_spacer_only_while_it_is_shorter_than_its_travel() {
        assert_eq!(fitted(2, 66, 10), switch(2, 10));
        assert_eq!(fitted(2, 66, 66), switch(2, 0));
    }

    #[test]
    fn keys_without_a_switch_use_the_factory_switch_of_the_unit() {
        assert_eq!(
            factory_switch(&capabilities(None)),
            switch(FACTORY_MODEL, 0)
        );
        assert_eq!(
            factory_switch(&capabilities(Some(9))),
            switch(EDITION_FACTORY_MODEL, 0)
        );
    }

    #[test]
    fn a_shared_value_needs_every_value_to_agree() {
        assert_eq!(same([4, 4, 4]), Some(4));
        assert_eq!(same([4, 5]), None);
        assert_eq!(same(Vec::<u8>::new()), None);
    }
}
