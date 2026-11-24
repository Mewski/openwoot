mod catalog;

use std::{collections::BTreeSet, mem};

use gpui_kit::{
    App, AppContext as _, Context, Div, Entity, FontWeight, Hsla, IntoElement, Modifiers,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window,
    component::{
        ActiveTheme as _, Disableable as _, Sizable as _,
        button::Button,
        h_flex,
        select::{SearchableVec, Select, SelectEvent, SelectGroup, SelectItem, SelectState},
        slider::{Slider, SliderEvent, SliderState},
        tab::{Tab, TabBar},
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
    page::Page,
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    usage,
};

const MILLIMETRES_PER_STEP: f32 = 0.05;
const READOUT_WIDTH: f32 = 60.0;
const MENU_HEIGHT: f32 = 360.0;
const HUE_STEP: f32 = 0.618_034;
const MODEL_SATURATION: f32 = 0.45;
const MODEL_LIGHTNESS: f32 = 0.38;
const FACTORY_MODEL: u8 = 0;
const EDITION_FACTORY_MODEL: u8 = 1;
const FIRST_BRAND: &str = "Wooting";
const OTHER_BRAND: &str = "Other";
const LABELS: [KeyLabels; 2] = [KeyLabels::Switch, KeyLabels::Travel];

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
                self.problem = Some(invalid.to_string().into());
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

    fn heading(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let selected_own = keyboard.switches.as_ref().is_some_and(|switches| {
            self.selection
                .iter()
                .any(|key| switches.keys.get(key).copied().flatten().is_some())
        });
        panel::heading(
            Page::Switches.title(),
            Some("Assign the switches installed in your keyboard for precise key travel.".into()),
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    TabBar::new("switch-labels")
                        .segmented()
                        .small()
                        .children([Tab::new().label("Switch"), Tab::new().label("Travel")])
                        .when_some(
                            LABELS.iter().position(|labels| *labels == self.labels),
                            TabBar::selected_index,
                        )
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            if let Some(labels) = LABELS.get(*index) {
                                this.show_labels(*labels, cx);
                            }
                        })),
                )
                .when(selected_own, |this| {
                    this.child(
                        Button::new("use-default-switch")
                            .small()
                            .label("Use default")
                            .tooltip("Use the default switch for the selected keys")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.use_default_switch(window, cx);
                            })),
                    )
                })
                .child(
                    Button::new("select-all")
                        .small()
                        .label("Select all keys")
                        .on_click(cx.listener(|this, _, window, cx| this.select_all(window, cx))),
                )
                .child(
                    Button::new("clear-selection")
                        .small()
                        .label("Discard selection")
                        .disabled(self.selection.is_empty())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.clear_selection(window, cx);
                        })),
                ),
            cx,
        )
    }

    fn keyboard(&self, keyboard: &Keyboard, window: &Window, cx: &Context<Self>) -> Div {
        let labels = self.labels;
        let page = cx.weak_entity();
        v_flex()
            .w_full()
            .gap_4()
            .child(panel::stage(
                KeyboardView::new("switch-keys", keyboard, |key| face(keyboard, labels, key))
                    .selection(&self.selection)
                    .fit_width(panel::content_width(window))
                    .on_key_click(move |key, event, window, cx| {
                        page.update(cx, |page, cx| {
                            page.click_key(key, event.modifiers(), window, cx);
                        })
                        .ok();
                    }),
            ))
            .when(self.selection.is_empty(), |this| {
                this.child(h_flex().w_full().justify_center().child(panel::label(
                    "Select keys to give them their own switch",
                    cx,
                )))
            })
    }

    fn switch_card(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let targets = targets(keyboard, &self.selection);
        let travel = same(targets.iter().map(|switch| {
            keyboard
                .models
                .get(usize::from(switch.model))
                .map(|model| model.travel)
        }))
        .flatten()
        .map_or_else(
            || "Mixed".into(),
            |travel| millimetres_text(millimetres(travel)),
        );
        let (description, scope) = if self.selection.is_empty() {
            (
                "Keys without their own switch use the default switch.",
                SharedString::from("Default switch"),
            )
        } else {
            (
                "Choose the switch installed in the selected keys.",
                keys_text(self.selection.len()),
            )
        };
        panel::section("Switch", Some(description.into()), cx)
            .child(scope_label("Changing switch for", scope, cx))
            .child(panel::field(
                "Model",
                Select::new(&self.model_select)
                    .w_full()
                    .placeholder("Mixed")
                    .search_placeholder("Search switches")
                    .menu_max_h(px(MENU_HEIGHT)),
                cx,
            ))
            .child(panel::field(
                "Total travel",
                div()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(travel),
                cx,
            ))
    }

    fn spacer_card(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let targets = targets(keyboard, &self.selection);
        let size = if same(targets.iter().map(|switch| switch.travel_offset)).is_some() {
            millimetres_text(self.spacer.read(cx).value().end())
        } else {
            "Mixed".into()
        };
        let tuned = keyboard.switches.as_ref().map_or(0, tuned_pairs);
        panel::section(
            "Spacers",
            Some("Enter the exact size of the spacers installed in these keys.".into()),
            cx,
        )
        .child(panel::field(
            "Size",
            h_flex()
                .w_full()
                .gap_3()
                .items_center()
                .child(div().flex_1().min_w_0().child(Slider::new(&self.spacer)))
                .child(readout_text(size, cx)),
            cx,
        ))
        .child(panel::field(
            "Spacer combinations",
            div()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(format!("{tuned} of {MAX_TUNED} used")),
            cx,
        ))
        .when_some(self.problem.clone(), |this, problem| {
            this.child(div().text_xs().text_color(cx.theme().danger).child(problem))
        })
    }
}

impl Render for SwitchesPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = panel::page();
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return page;
        };
        if keyboard.switches.is_none() {
            return page.child(panel::note("Switches unavailable", cx));
        }
        page.child(self.keyboard(keyboard, window, cx))
            .child(self.heading(keyboard, cx))
            .child(
                h_flex()
                    .w_full()
                    .items_stretch()
                    .gap_4()
                    .child(self.switch_card(keyboard, cx).flex_1().min_w_0())
                    .child(self.spacer_card(keyboard, cx).flex_1().min_w_0()),
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
            travel: millimetres_text(millimetres(entry.travel)),
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
    let detail = match labels {
        KeyLabels::Switch => Some(short_name(switch.model)),
        KeyLabels::Travel => keyboard.models.get(usize::from(switch.model)).map(|model| {
            let travel = millimetres(model.travel.saturating_sub(switch.travel_offset));
            SharedString::from(format!("{travel:.2}"))
        }),
    };
    KeyFace {
        legend,
        detail,
        fill: switches
            .keys
            .get(key)
            .copied()
            .flatten()
            .map(|own| model_color(own.model)),
        travel: None,
        dimmed: false,
    }
}

fn scope_label(prefix: &'static str, value: SharedString, cx: &Context<SwitchesPage>) -> Div {
    v_flex().gap_1().child(panel::label(prefix, cx)).child(
        div()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().primary)
            .child(value),
    )
}

fn keys_text(count: usize) -> SharedString {
    if count == 1 {
        "1 key".into()
    } else {
        format!("{count} keys").into()
    }
}

fn readout_text(text: SharedString, cx: &Context<SwitchesPage>) -> Div {
    div()
        .w(px(READOUT_WIDTH))
        .text_right()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text)
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

fn millimetres_text(value: f32) -> SharedString {
    format!("{value:.2} mm").into()
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
