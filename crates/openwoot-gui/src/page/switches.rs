mod catalog;
mod fitting;
mod glyph;

use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    assets::IconName,
    component::{
        ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex,
        input::{Input, InputEvent, InputState},
        notification::Notification,
        tooltip::Tooltip,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    configuration::Validate as _,
    endpoints::SetSwitches,
    switches::{self, MAX_TUNED, Model, Switch},
};

use super::{
    Page,
    performance::{Committed, DistanceField, Shown, Span, actions, millimetres_text, stage},
};
use crate::{
    dropdown::{Dropdown, Item},
    keyboard::{Glyph, KeyFace, KeyboardView, Selection},
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    switch::Switch as Toggle,
    usage,
};

const FIRST_BRAND: &str = "Wooting";
const ALL_BRANDS: &str = "All brands";
const SPACER_DEFAULT_MM: f32 = 0.5;
const GRID_COLUMNS: u16 = 4;
const CARD_HEIGHT: f32 = 128.0;
const CARD_GLYPH: f32 = 44.0;
const SPACERS_WIDTH: f32 = 340.0;
const KEY_GLYPH_SHARE: f32 = 0.62;
const TOOLTIP_GLYPH: f32 = 18.0;
const BRAND_WIDTH: f32 = 200.0;
const PROMPT: &str = "To adjust your switches, select one or more keys first";
const HELP: &str = "Assign the switches installed in your keyboard. Each switch brings its calibrated analog curve and total travel, so the keyboard knows the precise position of every key.";
const SPACERS_HELP: &str = "Spacers are thin inserts inside your switches that physically shorten the total travel, making keystrokes feel quicker and more responsive.";

pub(crate) struct SwitchesPage {
    store: Entity<Store>,
    selection: Selection,
    search: Entity<InputState>,
    brand: Option<&'static str>,
    spacer: Entity<DistanceField>,
    spacers_hovered: bool,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spacers {
    Off,
    On,
    Mixed,
}

#[derive(Clone, Copy)]
struct SwitchCard {
    model: u8,
    product: &'static catalog::Product,
    travel: f32,
    current: bool,
    selectable: bool,
    housing: Hsla,
}

impl SwitchesPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search for a switch"));
        let spacer = cx.new(|cx| DistanceField::plain(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected => {
                    this.selection.clear();
                    this.sync(window, cx);
                }
                StoreEvent::Disconnected
                | StoreEvent::ProfileChanged
                | StoreEvent::Reloaded
                | StoreEvent::Failed(_) => {}
            }),
            cx.observe_in(&store, window, |this, _, window, cx| this.sync(window, cx)),
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe_in(
                &spacer,
                window,
                |this, _, Committed(millimetres): &Committed, window, cx| {
                    this.set_spacer_size(*millimetres, window, cx);
                },
            ),
        ];
        let mut page = Self {
            store,
            selection: Selection::default(),
            search,
            brand: None,
            spacer,
            spacers_hovered: false,
            _subscriptions: subscriptions,
        };
        page.sync(window, cx);
        page
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = {
            let Some(keyboard) = self.store.read(cx).keyboard() else {
                return;
            };
            let targets = self.targets(keyboard);
            let travel = targets
                .iter()
                .filter_map(|switch| keyboard.models.get(usize::from(switch.model)))
                .map(|model| fitting::millimetres(model.travel))
                .reduce(f32::min)
                .unwrap_or(0.0);
            let fitted: Vec<f32> = targets
                .iter()
                .filter(|switch| switch.is_tuned())
                .map(|switch| fitting::millimetres(switch.travel_offset))
                .collect();
            let mixed = fitted.iter().any(|size| {
                millimetres_text(*size)
                    != fitted
                        .first()
                        .map_or_default(|first| millimetres_text(*first))
            });
            Shown {
                millimetres: fitted.first().copied().unwrap_or(SPACER_DEFAULT_MM),
                span: Span::spacer(travel),
                default: Some(SPACER_DEFAULT_MM),
                mixed,
                disabled: spacers(&targets) != Spacers::On || self.selection.is_empty(),
            }
        };
        self.spacer
            .update(cx, |field, cx| field.show(shown, window, cx));
        cx.notify();
    }

    fn targets(&self, keyboard: &Keyboard) -> Vec<Switch> {
        let Some(settings) = &keyboard.switches else {
            return Vec::new();
        };
        let factory = fitting::factory(&keyboard.capabilities);
        self.selection
            .iter()
            .map(|key| fitting::of_key(settings, factory, key))
            .collect()
    }

    fn shown_switches(&self, keyboard: &Keyboard) -> Vec<Switch> {
        if !self.selection.is_empty() {
            return self.targets(keyboard);
        }
        let Some(settings) = &keyboard.switches else {
            return Vec::new();
        };
        let factory = fitting::factory(&keyboard.capabilities);
        (0..keyboard.key_count())
            .filter(|key| keyboard.has_key(*key))
            .map(|key| fitting::of_key(settings, factory, key))
            .collect()
    }

    fn toggle_key(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selection.toggle(key);
        self.sync(window, cx);
    }

    fn sweep_key(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selection.contains(key) {
            self.selection.insert(key);
            self.sync(window, cx);
        }
    }

    fn select_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(keyboard) = self.store.read(cx).keyboard() {
            self.selection.select_all(keyboard);
        }
        self.sync(window, cx);
    }

    fn clear_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selection.clear();
        self.sync(window, cx);
    }

    fn filter_brand(&mut self, brand: Option<&'static str>, cx: &mut Context<Self>) {
        self.brand = brand;
        cx.notify();
    }

    fn hover_spacers(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.spacers_hovered != hovered {
            self.spacers_hovered = hovered;
            cx.notify();
        }
    }

    fn assign_model(&mut self, model: u8, window: &mut Window, cx: &mut Context<Self>) {
        self.change(
            |settings, keys, factory, models| {
                fitting::assign(settings, keys, factory, model, models);
            },
            window,
            cx,
        );
    }

    fn set_spacers(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.change(
            |settings, keys, factory, models| {
                fitting::set_spacers(settings, keys, factory, |current| {
                    if !enabled {
                        0
                    } else if current.is_tuned() {
                        current.travel_offset
                    } else {
                        spacer_steps(SPACER_DEFAULT_MM, current, models)
                    }
                });
            },
            window,
            cx,
        );
    }

    fn set_spacer_size(&mut self, millimetres: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.change(
            |settings, keys, factory, models| {
                fitting::set_spacers(settings, keys, factory, |current| {
                    spacer_steps(millimetres, current, models)
                });
            },
            window,
            cx,
        );
    }

    fn change(
        &mut self,
        edit: impl FnOnce(&mut switches::Settings, &[usize], Switch, &[Model]),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selection.is_empty() {
            return;
        }
        let checked = {
            let Some(keyboard) = self.store.read(cx).keyboard() else {
                return;
            };
            let Some(before) = keyboard.switches.clone() else {
                return;
            };
            let factory = fitting::factory(&keyboard.capabilities);
            let keys: Vec<usize> = self.selection.iter().collect();
            let present: Vec<usize> = (0..keyboard.key_count())
                .filter(|key| keyboard.has_key(*key))
                .collect();
            let mut after = before.clone();
            edit(&mut after, &keys, factory, &keyboard.models);
            fitting::fold(&mut after, &present, factory);
            after
                .validate(&keyboard.capabilities)
                .and_then(|()| after.check_models(&keyboard.models))
                .map(|()| (before, after))
        };
        match checked {
            Err(invalid) => {
                let message = format!("could not change the switches: {invalid}");
                window.push_notification(Notification::error(message), cx);
                self.sync(window, cx);
            }
            Ok((before, after)) if before == after => {}
            Ok((before, after)) => {
                let applied = self.store.update(cx, |store, cx| {
                    store.change::<SetSwitches>(&before, after, Scope::Keyboard, cx)
                });
                cx.spawn_in(window, async move |this, cx| {
                    let applied = applied.await.is_ok();
                    this.update_in(cx, |page, window, cx| {
                        if applied {
                            page.store
                                .update(cx, |store, cx| store.reload_switches(cx))
                                .detach();
                        } else {
                            page.sync(window, cx);
                        }
                    })
                    .ok();
                })
                .detach();
            }
        }
    }

    fn keyboard(&self, keyboard: &Keyboard, window: &Window, cx: &Context<Self>) -> Div {
        let housing = cx.theme().muted_foreground.opacity(0.45);
        let page = cx.weak_entity();
        let swept = cx.weak_entity();
        let view = KeyboardView::new("switch-keys", keyboard, |key| {
            self.face(keyboard, key, housing)
        })
        .selection(&self.selection)
        .fit_width(panel::content_width(window))
        .on_key_click(move |key, _, window, cx| {
            page.update(cx, |page, cx| page.toggle_key(key, window, cx))
                .ok();
        })
        .on_key_sweep(move |key, window, cx| {
            swept
                .update(cx, |page, cx| page.sweep_key(key, window, cx))
                .ok();
        });
        stage(view, self.selection.len(), PROMPT, cx)
    }

    fn face(&self, keyboard: &Keyboard, key: usize, housing: Hsla) -> KeyFace {
        let legend = keyboard
            .legends
            .get(key)
            .map_or_default(|action| SharedString::from(usage::legend(*action)));
        let Some(settings) = &keyboard.switches else {
            return KeyFace {
                legend,
                ..KeyFace::default()
            };
        };
        let switch = fitting::of_key(settings, fitting::factory(&keyboard.capabilities), key);
        if self.spacers_hovered {
            return KeyFace {
                legend,
                detail: switch
                    .is_tuned()
                    .then(|| millimetres_text(fitting::millimetres(switch.travel_offset))),
                ..KeyFace::default()
            };
        }
        let stem = glyph::stem_color(switch.model);
        KeyFace {
            legend,
            glyph: Some(Glyph::new(move |height| {
                glyph::switch(height * KEY_GLYPH_SHARE, stem, housing).into_any_element()
            })),
            tooltip: Some(product_name(switch.model)),
            ..KeyFace::default()
        }
    }

    fn assign_card(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let query = self.search.read(cx).value().to_lowercase();
        let models = models(keyboard);
        let brands = brands(&models);
        let shown: Vec<(u8, &'static catalog::Product)> = models
            .into_iter()
            .filter(|(_, product)| self.brand.is_none_or(|brand| product.brand == brand))
            .filter(|(_, product)| query.is_empty() || matches(product, &query))
            .collect();
        panel::section(
            "Assign switches",
            Some("Select keys on the keyboard, then pick the switches installed in them.".into()),
            cx,
        )
        .child(
            h_flex()
                .w_full()
                .gap_2()
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&self.search)
                            .prefix(Icon::new(IconName::Search).small())
                            .cleanable(true),
                    ),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(BRAND_WIDTH))
                        .child(self.brand_filter(brands, cx)),
                ),
        )
        .child(self.grid(keyboard, shown, cx))
    }

    fn brand_filter(&self, brands: Vec<&'static str>, cx: &Context<Self>) -> Dropdown {
        let page = cx.weak_entity();
        let selected = self
            .brand
            .and_then(|brand| brands.iter().position(|known| *known == brand))
            .map(|index| index + 1);
        Dropdown::new("brand-filter")
            .placeholder("Filter by brand")
            .items(
                std::iter::once(Item::new(ALL_BRANDS))
                    .chain(brands.iter().map(|brand| Item::new(*brand))),
            )
            .selected(selected)
            .on_select(move |index, _, cx| {
                let brand = index
                    .checked_sub(1)
                    .and_then(|index| brands.get(index).copied());
                page.update(cx, |page, cx| page.filter_brand(brand, cx))
                    .ok();
            })
    }

    fn grid(
        &self,
        keyboard: &Keyboard,
        shown: Vec<(u8, &'static catalog::Product)>,
        cx: &Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        if shown.is_empty() {
            return v_flex()
                .gap_1()
                .py_4()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child("Switch not found"),
                )
                .child(
                    div().text_sm().text_color(theme.muted_foreground).child(
                        "Pick a switch of the same brand, or one with the same total travel.",
                    ),
                );
        }
        let current = same(
            self.shown_switches(keyboard)
                .iter()
                .map(|switch| switch.model),
        );
        let selectable = !self.selection.is_empty();
        let housing = theme.muted_foreground.opacity(0.45);
        div()
            .grid()
            .grid_cols(GRID_COLUMNS)
            .gap_2()
            .children(shown.into_iter().map(|(model, product)| {
                SwitchCard {
                    model,
                    product,
                    travel: keyboard
                        .models
                        .get(usize::from(model))
                        .map_or(0.0, |entry| fitting::millimetres(entry.travel)),
                    current: current == Some(model),
                    selectable,
                    housing,
                }
                .render(cx)
            }))
    }

    fn spacers_card(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let targets = self.targets(keyboard);
        let state = spacers(&targets);
        let selectable = !self.selection.is_empty();
        let tuned = keyboard.switches.as_ref().map_or(0, fitting::tuned_pairs);
        let page = cx.weak_entity();
        panel::section_with_help(
            "Spacers",
            SPACERS_HELP,
            Some(
                "If you have spacers installed, select those keys and enter their exact size."
                    .into(),
            ),
            Some(
                Toggle::new("spacers")
                    .checked(state == Spacers::On)
                    .mixed(state == Spacers::Mixed)
                    .disabled(!selectable)
                    .on_change(move |enabled: &bool, window, cx| {
                        page.update(cx, |page, cx| page.set_spacers(*enabled, window, cx))
                            .ok();
                    })
                    .into_any_element(),
            ),
            cx,
        )
        .child(panel::field("Size", self.spacer.clone(), cx))
        .child(panel::note(
            format!("{tuned} of {MAX_TUNED} spacer combinations used"),
            cx,
        ))
        .when(!selectable, |this| this.opacity(0.5))
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
        let selecting = cx.weak_entity();
        let discarding = cx.weak_entity();
        let total = (0..keyboard.key_count())
            .filter(|key| keyboard.has_key(*key))
            .count();
        panel::page_with_stage(
            "switch-page",
            self.keyboard(keyboard, window, cx),
            v_flex()
                .w_full()
                .gap_5()
                .child(panel::heading(
                    Page::Switches.title(),
                    Some(HELP.into()),
                    actions(
                        self.selection.len(),
                        total,
                        move |window, cx| {
                            selecting
                                .update(cx, |page, cx| page.select_all(window, cx))
                                .ok();
                        },
                        move |window, cx| {
                            discarding
                                .update(cx, |page, cx| page.clear_selection(window, cx))
                                .ok();
                        },
                    ),
                    cx,
                ))
                .child(
                    h_flex()
                        .w_full()
                        .items_start()
                        .gap_4()
                        .child(self.assign_card(keyboard, cx).flex_1().min_w_0())
                        .child(
                            self.spacers_card(keyboard, cx)
                                .id("spacers-card")
                                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                    this.hover_spacers(*hovered, cx);
                                }))
                                .flex_none()
                                .w(px(SPACERS_WIDTH)),
                        ),
                ),
        )
    }
}

impl SwitchCard {
    fn render(self, cx: &Context<SwitchesPage>) -> AnyElement {
        let theme = cx.theme();
        let Self {
            model,
            product,
            travel,
            current,
            selectable,
            housing,
        } = self;
        let hover = theme.muted_foreground;
        let stem = glyph::stem_color(model);
        div()
            .id(("switch-card", u32::from(model)))
            .w_full()
            .h(px(CARD_HEIGHT))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .px_2()
            .rounded(theme.radius)
            .bg(theme.tab_active)
            .border_1()
            .border_color(if current { theme.primary } else { theme.border })
            .when(selectable, |this| {
                this.cursor_pointer()
                    .hover(move |style| {
                        if current {
                            style
                        } else {
                            style.border_color(hover)
                        }
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.assign_model(model, window, cx);
                    }))
            })
            .child(glyph::switch(px(CARD_GLYPH), stem, housing))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .truncate()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .when(current, |this| this.font_weight(FontWeight::SEMIBOLD))
                    .child(product.brand),
            )
            .child(
                div()
                    .w_full()
                    .text_center()
                    .truncate()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child(product.name),
            )
            .tooltip(move |window, cx| {
                Tooltip::element(move |_, cx| switch_tooltip(product, travel, stem, housing, cx))
                    .build(window, cx)
            })
            .into_any_element()
    }
}

fn switch_tooltip(
    product: &'static catalog::Product,
    travel: f32,
    stem: Hsla,
    housing: Hsla,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    v_flex()
        .gap_2()
        .py_1()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child(format!("{} {}", product.brand, product.name)),
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(IconName::ArrowUpDown).small())
                .child(format!("Total travel: {travel:.1} mm")),
        )
        .when(!product.variants.is_empty(), |this| {
            this.child(panel::label("Supported variants", cx)).children(
                product.variants.iter().map(|variant| {
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(glyph::switch(px(TOOLTIP_GLYPH), stem, housing))
                        .child(div().text_sm().text_color(theme.foreground).child(*variant))
                }),
            )
        })
}

fn spacers(targets: &[Switch]) -> Spacers {
    match same(targets.iter().map(Switch::is_tuned)) {
        Some(true) => Spacers::On,
        Some(false) => Spacers::Off,
        None if targets.is_empty() => Spacers::Off,
        None => Spacers::Mixed,
    }
}

fn spacer_steps(millimetres: f32, switch: Switch, models: &[Model]) -> u8 {
    let travel = models
        .get(usize::from(switch.model))
        .map_or(0.0, |model| fitting::millimetres(model.travel));
    fitting::steps_of(Span::spacer(travel).snap(millimetres))
}

fn models(keyboard: &Keyboard) -> Vec<(u8, &'static catalog::Product)> {
    let available = usize::from(keyboard.capabilities.switch_models);
    let mut models: Vec<(u8, &'static catalog::Product)> = (0..=u8::MAX)
        .zip(&keyboard.models)
        .take(available)
        .filter_map(|(model, _)| catalog::product(model).map(|product| (model, product)))
        .collect();
    models.sort_by_key(|(model, product)| {
        (
            product.brand != FIRST_BRAND,
            product.brand.to_lowercase(),
            *model,
        )
    });
    models
}

fn brands(models: &[(u8, &'static catalog::Product)]) -> Vec<&'static str> {
    let mut brands: Vec<&'static str> = Vec::new();
    for (_, product) in models {
        if !brands.contains(&product.brand) {
            brands.push(product.brand);
        }
    }
    brands
}

fn matches(product: &catalog::Product, query: &str) -> bool {
    product.brand.to_lowercase().contains(query)
        || product.name.to_lowercase().contains(query)
        || product
            .variants
            .iter()
            .any(|variant| variant.to_lowercase().contains(query))
}

fn product_name(model: u8) -> SharedString {
    catalog::product(model).map_or_else(
        || format!("Switch model {model}").into(),
        |product| format!("{} {}", product.brand, product.name).into(),
    )
}

fn same<T: PartialEq>(values: impl IntoIterator<Item = T>) -> Option<T> {
    let mut values = values.into_iter();
    let first = values.next()?;
    values.all(|value| value == first).then_some(first)
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

    #[test]
    fn spacers_read_on_off_or_mixed_over_the_selected_keys() {
        assert_eq!(spacers(&[]), Spacers::Off);
        assert_eq!(spacers(&[switch(0, 10), switch(2, 4)]), Spacers::On);
        assert_eq!(spacers(&[switch(0, 0)]), Spacers::Off);
        assert_eq!(spacers(&[switch(0, 10), switch(0, 0)]), Spacers::Mixed);
    }

    #[test]
    fn a_spacer_stays_a_millimetre_shorter_than_the_switch() {
        let models = [Model {
            travel: 80,
            group: 0,
        }];
        assert_eq!(spacer_steps(0.5, switch(0, 0), &models), 10);
        assert_eq!(spacer_steps(9.0, switch(0, 0), &models), 60);
    }

    #[test]
    fn search_finds_brands_names_and_variants() {
        let product = catalog::product(1).expect("the catalogue lists the Lekker Tikken");
        assert!(matches(product, "wooting"));
        assert!(matches(product, "tikken"));
        assert!(matches(product, "tenz"));
        assert!(!matches(product, "jade"));
    }

    #[test]
    fn brands_keep_their_first_appearance_order() {
        let models: Vec<(u8, &'static catalog::Product)> = [0, 2, 1, 3]
            .into_iter()
            .filter_map(|model| catalog::product(model).map(|product| (model, product)))
            .collect();
        assert_eq!(brands(&models), ["Wooting", "Gateron", "GEON"]);
    }

    #[test]
    fn unknown_models_still_have_a_name() {
        assert_eq!(product_name(200), "Switch model 200");
        assert_eq!(product_name(2), "Gateron Jade");
    }
}
