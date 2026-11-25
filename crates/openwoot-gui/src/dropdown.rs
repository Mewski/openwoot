use std::rc::Rc;

use gpui_kit::{
    Anchor, AnyElement, App, Bounds, BoxShadow, Context, Div, ElementId, Entity, FocusHandle,
    FontWeight, Global, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, MouseDownEvent,
    ParentElement as _, Pixels, Point, RenderOnce, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, anchored,
    assets::IconName,
    base::{
        POPUP_PRIORITY,
        actions::{Cancel, Confirm, SelectDown, SelectUp},
    },
    canvas,
    component::{ActiveTheme as _, Icon},
    deferred, div, point,
    prelude::FluentBuilder as _,
    px,
};

use crate::{panel, theme};

const ROW_HEIGHT: f32 = 48.0;
const ICON_ROW_HEIGHT: f32 = 42.0;
const ROW_PADDING: f32 = 16.0;
const ROW_TEXT: f32 = 16.0;
const ROW_LINE: f32 = 24.0;
const ICON_ROW_LINE: f32 = 16.0;
const ROW_ICON: f32 = 18.0;
const ROW_ICON_GAP: f32 = 12.0;
const SHORTCUT_GAP: f32 = 32.0;
const DIVIDER: f32 = 1.0;
const DIVIDER_OPACITY: f32 = 0.6;
const DETAIL_TEXT: f32 = 14.0;
const MENU_GAP: f32 = 8.0;
const MENU_PADDING: f32 = 8.0;
const MENU_RADIUS: f32 = 8.0;
const MENU_MAX_HEIGHT: f32 = 360.0;
const MENU_MIN_WIDTH: f32 = 200.0;
const CONTEXT_MIN_WIDTH: f32 = 224.0;
const MENU_SHADOW_Y: f32 = 8.0;
const MENU_SHADOW_BLUR: f32 = 12.0;
const MENU_SHADOW_SPREAD: f32 = -2.0;
const MENU_SHADOW_ALPHA: f32 = 0.24;
const WINDOW_MARGIN: f32 = 8.0;
const FIELD_HEIGHT: f32 = 40.0;
const SMALL_FIELD_HEIGHT: f32 = 32.0;
const FIELD_PADDING: f32 = 8.0;
const FIELD_RADIUS: f32 = 6.0;
const FIELD_TEXT: f32 = 16.0;
const FIELD_LINE: f32 = 19.2;
const SMALL_FIELD_TEXT: f32 = 12.8;
const SUBTLE_FIELD_HEIGHT: f32 = 36.0;
const SUBTLE_FIELD_TEXT: f32 = 14.0;
const SUBTLE_FIELD_LINE: f32 = 16.8;
const CHEVRON: f32 = 16.0;
const OUTLINE: f32 = 1.0;
const DISABLED_OPACITY: f32 = 0.4;

type Select = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type Content = Rc<dyn Fn(&Close, &mut Window, &mut App) -> AnyElement>;
type TriggerFace = Box<dyn FnOnce(bool, &App) -> AnyElement>;

#[derive(Debug, Clone, Default)]
pub(crate) struct Item {
    label: SharedString,
    detail: Option<SharedString>,
    shortcut: Option<u8>,
    icon: Option<IconName>,
    disabled: bool,
    danger: bool,
    divided: bool,
}

#[derive(IntoElement)]
pub(crate) struct Dropdown {
    id: ElementId,
    trigger: Trigger,
    body: Body,
    selected: Option<usize>,
    disabled: bool,
    look: FieldLook,
    menu_width: Option<Pixels>,
    align_end: bool,
    side: Side,
    gap: Option<Pixels>,
}

#[derive(Clone)]
pub(crate) struct Close {
    state: Entity<State>,
}

enum Trigger {
    Field { placeholder: SharedString },
    Custom(TriggerFace),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Below,
    BelowCentre,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldLook {
    Regular,
    Small,
    Subtle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Menu {
    Select,
    Context,
    Custom,
}

enum Body {
    Items {
        items: Vec<Item>,
        on_select: Option<Select>,
    },
    Custom(Content),
}

struct Choices {
    enabled: Vec<bool>,
    on_select: Option<Select>,
}

#[derive(Debug, Clone, Copy)]
struct RowSize {
    height: f32,
    line: f32,
}

struct State {
    open: bool,
    trigger: Bounds<Pixels>,
    highlighted: Option<usize>,
    focus: FocusHandle,
    layer: usize,
}

#[derive(Default)]
struct OpenMenus {
    menus: Vec<(WeakEntity<State>, Bounds<Pixels>)>,
}

impl Global for OpenMenus {}

impl OpenMenus {
    fn record(&mut self, menu: WeakEntity<State>, bounds: Bounds<Pixels>) {
        self.menus
            .retain(|(known, _)| known != &menu && known.upgrade().is_some());
        self.menus.push((menu, bounds));
    }

    fn covers(&self, asking: &WeakEntity<State>, position: Point<Pixels>, cx: &App) -> bool {
        self.menus.iter().any(|(menu, bounds)| {
            menu != asking
                && bounds.contains(&position)
                && menu.upgrade().is_some_and(|state| state.read(cx).open)
        })
    }

    fn open_count(&self, cx: &App) -> usize {
        self.menus
            .iter()
            .filter(|(menu, _)| menu.upgrade().is_some_and(|state| state.read(cx).open))
            .count()
    }
}

impl Item {
    pub(crate) fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            ..Self::default()
        }
    }

    pub(crate) fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub(crate) fn shortcut(mut self, usage: u8) -> Self {
        self.shortcut = Some(usage);
        self
    }

    pub(crate) fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub(crate) fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    pub(crate) fn divider_before(mut self) -> Self {
        self.divided = true;
        self
    }
}

impl Dropdown {
    pub(crate) fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            trigger: Trigger::Field {
                placeholder: SharedString::default(),
            },
            body: Body::Items {
                items: Vec::new(),
                on_select: None,
            },
            selected: None,
            disabled: false,
            look: FieldLook::Regular,
            menu_width: None,
            align_end: false,
            side: Side::Below,
            gap: None,
        }
    }

    pub(crate) fn items(mut self, items: impl IntoIterator<Item = Item>) -> Self {
        let on_select = match self.body {
            Body::Items { on_select, .. } => on_select,
            Body::Custom(_) => None,
        };
        self.body = Body::Items {
            items: items.into_iter().collect(),
            on_select,
        };
        self
    }

    pub(crate) fn on_select(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        if let Body::Items { on_select, .. } = &mut self.body {
            *on_select = Some(Rc::new(handler));
        }
        self
    }

    pub(crate) fn content(
        mut self,
        content: impl Fn(&Close, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.body = Body::Custom(Rc::new(content));
        self
    }

    pub(crate) fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    pub(crate) fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.trigger = Trigger::Field {
            placeholder: placeholder.into(),
        };
        self
    }

    pub(crate) fn trigger(mut self, face: impl FnOnce(bool, &App) -> AnyElement + 'static) -> Self {
        self.trigger = Trigger::Custom(Box::new(face));
        self
    }

    pub(crate) fn menu_width(mut self, width: Pixels) -> Self {
        self.menu_width = Some(width);
        self
    }

    pub(crate) fn below_centre(mut self, gap: Pixels) -> Self {
        self.side = Side::BelowCentre;
        self.gap = Some(gap);
        self
    }

    pub(crate) fn right(mut self, gap: Pixels) -> Self {
        self.side = Side::Right;
        self.gap = Some(gap);
        self
    }

    pub(crate) fn align_end(mut self) -> Self {
        self.align_end = true;
        self
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub(crate) fn small(mut self) -> Self {
        self.look = FieldLook::Small;
        self
    }

    pub(crate) fn subtle(mut self) -> Self {
        self.look = FieldLook::Subtle;
        self
    }
}

impl Close {
    pub(crate) fn close(&self, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            state.open = false;
            cx.notify();
        });
    }
}

fn toggle(state: &Entity<State>, window: &mut Window, cx: &mut App) {
    let layer = cx
        .try_global::<OpenMenus>()
        .map_or(0, |menus| menus.open_count(cx));
    state.update(cx, |state, cx| {
        state.open = !state.open;
        state.layer = layer;
        state.highlighted = None;
        if state.open {
            window.focus(&state.focus, cx);
        }
        cx.notify();
    });
}

impl RenderOnce for Dropdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| State {
            open: false,
            trigger: Bounds::default(),
            highlighted: None,
            focus: cx.focus_handle(),
            layer: 0,
        });
        let open = state.read(cx).open && !self.disabled;
        let label = match &self.body {
            Body::Items { items, .. } => self
                .selected
                .and_then(|index| items.get(index))
                .map(|item| item.label.clone()),
            Body::Custom(_) => None,
        };
        let kind = match (&self.trigger, &self.body) {
            (_, Body::Custom(_)) => Menu::Custom,
            (Trigger::Field { .. }, Body::Items { .. }) => Menu::Select,
            (Trigger::Custom(_), Body::Items { .. }) => Menu::Context,
        };
        let face = match self.trigger {
            Trigger::Field { placeholder } => {
                field(label, placeholder, (open, self.disabled, self.look), cx)
            }
            Trigger::Custom(face) => face(open, cx),
        };
        let measured = state.clone();
        let toggled = state.clone();
        let trigger = div()
            .id(self.id.clone())
            .relative()
            .w_full()
            .when(!self.disabled, gpui_kit::Styled::cursor_pointer)
            .child(face)
            .child(
                canvas(
                    move |bounds, _, cx| {
                        measured.update(cx, |state, _| state.trigger = bounds);
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .when(!self.disabled, |this| {
                this.on_click(move |_, window, cx| toggle(&toggled, window, cx))
            });
        let menu = open.then(|| {
            let bounds = state.read(cx).trigger;
            let width = match (self.menu_width, kind) {
                (Some(width), _) => Some(width),
                (None, Menu::Context) => None,
                (None, Menu::Select | Menu::Custom) => {
                    Some(bounds.size.width.max(px(MENU_MIN_WIDTH)))
                }
            };
            let reach = width.unwrap_or(px(CONTEXT_MIN_WIDTH));
            let end = self.align_end
                || bounds.left() + reach > window.viewport_size().width - px(WINDOW_MARGIN);
            let (corner, offset) = self.side.placement(self.gap.unwrap_or(px(MENU_GAP)), end);
            let (content, choices) = match self.body {
                Body::Items { items, on_select } => {
                    let choices = Choices {
                        enabled: items.iter().map(|item| !item.disabled).collect(),
                        on_select,
                    };
                    let element = list(&state, items, choices.on_select.as_ref(), cx);
                    (element.into_any_element(), Some(choices))
                }
                Body::Custom(content) => (
                    content(
                        &Close {
                            state: state.clone(),
                        },
                        window,
                        cx,
                    ),
                    None,
                ),
            };
            let surface = surface(&state, width, kind, choices, cx);
            deferred(
                anchored()
                    .anchor(corner.0)
                    .attach(bounds, corner.1)
                    .offset(offset)
                    .snap_to_window_with_margin(px(WINDOW_MARGIN))
                    .child(surface.child(content)),
            )
            .with_priority(POPUP_PRIORITY + state.read(cx).layer)
        });
        div().w_full().child(trigger).children(menu)
    }
}

impl State {
    fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
    }
}

impl Side {
    fn placement(self, gap: Pixels, end: bool) -> ((Anchor, Anchor), Point<Pixels>) {
        match self {
            Self::Below if end => ((Anchor::TopRight, Anchor::BottomRight), point(px(0.0), gap)),
            Self::Below => ((Anchor::TopLeft, Anchor::BottomLeft), point(px(0.0), gap)),
            Self::BelowCentre => (
                (Anchor::TopCenter, Anchor::BottomCenter),
                point(px(0.0), gap),
            ),
            Self::Right => ((Anchor::TopLeft, Anchor::TopRight), point(gap, px(0.0))),
        }
    }
}

fn field(
    label: Option<SharedString>,
    placeholder: SharedString,
    (open, disabled, look): (bool, bool, FieldLook),
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let has_label = label.is_some();
    let outline = if open { theme.primary } else { theme.border };
    let hover_outline = theme.secondary_hover;
    let hover_background = theme.secondary;
    let (height, text, line, weight) = match look {
        FieldLook::Regular => (FIELD_HEIGHT, FIELD_TEXT, FIELD_LINE, FontWeight::NORMAL),
        FieldLook::Small => (
            SMALL_FIELD_HEIGHT,
            SMALL_FIELD_TEXT,
            FIELD_LINE,
            FontWeight::SEMIBOLD,
        ),
        FieldLook::Subtle => (
            SUBTLE_FIELD_HEIGHT,
            SUBTLE_FIELD_TEXT,
            SUBTLE_FIELD_LINE,
            FontWeight::NORMAL,
        ),
    };
    let outlined = look != FieldLook::Subtle;
    div()
        .group("dropdown-field")
        .relative()
        .h(px(height))
        .w_full()
        .px(px(FIELD_PADDING))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(FIELD_PADDING))
        .rounded(px(FIELD_RADIUS))
        .bg(if open { theme.secondary } else { theme.accent })
        .when(disabled, |this| this.opacity(DISABLED_OPACITY))
        .when(!disabled && !open, |this| {
            this.hover(move |style| style.bg(hover_background))
        })
        .when(outlined, |this| {
            this.child(
                div()
                    .absolute()
                    .top(px(-OUTLINE))
                    .left(px(-OUTLINE))
                    .right(px(-OUTLINE))
                    .bottom(px(-OUTLINE))
                    .rounded(px(FIELD_RADIUS + OUTLINE))
                    .border_1()
                    .border_color(outline)
                    .when(!disabled && !open, |this| {
                        this.group_hover("dropdown-field", move |style| {
                            style.border_color(hover_outline)
                        })
                    }),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(text))
                .line_height(px(line))
                .font_weight(weight)
                .text_color(if has_label {
                    theme.foreground
                } else {
                    theme.muted_foreground
                })
                .child(label.unwrap_or(placeholder)),
        )
        .child(
            Icon::new(if open {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            })
            .size(px(CHEVRON))
            .text_color(theme.foreground),
        )
        .into_any_element()
}

fn surface(
    state: &Entity<State>,
    width: Option<Pixels>,
    kind: Menu,
    choices: Option<Choices>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let dismissed = state.clone();
    let keyed = state.clone();
    let cancelled = state.clone();
    let confirmed = state.clone();
    let raised = state.clone();
    let lowered = state.clone();
    let choices = choices.map(Rc::new);
    let confirm_choices = choices.clone();
    let up_choices = choices.clone();
    let down_choices = choices.clone();
    let recorded = state.downgrade();
    let focus = state.read(cx).focus.clone();
    let listing = choices.is_some();
    let background = match kind {
        Menu::Context => theme::palette().overlay_hover,
        Menu::Select | Menu::Custom => theme.tab_active,
    };
    div()
        .id("dropdown-menu")
        .relative()
        .occlude()
        .track_focus(&focus)
        .map(|this| match width {
            Some(width) => this.w(width),
            None => this.min_w(px(CONTEXT_MIN_WIDTH)),
        })
        .when(listing, |this| {
            this.max_h(px(MENU_MAX_HEIGHT))
                .overflow_y_scroll()
                .py(px(MENU_PADDING))
        })
        .rounded(px(MENU_RADIUS))
        .bg(background)
        .border_1()
        .border_color(theme.border)
        .shadow(vec![BoxShadow {
            color: Hsla {
                a: MENU_SHADOW_ALPHA,
                ..theme.background
            },
            offset: point(px(0.0), px(MENU_SHADOW_Y)),
            blur_radius: px(MENU_SHADOW_BLUR),
            spread_radius: px(MENU_SHADOW_SPREAD),
            inset: false,
        }])
        .child(
            canvas(
                move |bounds, _, cx| {
                    cx.default_global::<OpenMenus>()
                        .record(recorded.clone(), bounds);
                },
                |_, (), _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .on_mouse_down_out(move |event: &MouseDownEvent, _, cx| {
            let inside_other = cx
                .try_global::<OpenMenus>()
                .is_some_and(|menus| menus.covers(&dismissed.downgrade(), event.position, cx));
            if inside_other {
                return;
            }
            dismissed.update(cx, |state, cx| {
                if !state.trigger.contains(&event.position) {
                    state.close(cx);
                }
            });
        })
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            press(&keyed, event, choices.as_deref(), window, cx);
        })
        .on_action(move |_: &Cancel, _, cx| dismiss(&cancelled, cx))
        .on_action(move |_: &Confirm, window, cx| {
            choose(&confirmed, confirm_choices.as_deref(), window, cx);
        })
        .on_action(move |_: &SelectUp, _, cx| {
            highlight(&raised, up_choices.as_deref(), false, cx);
        })
        .on_action(move |_: &SelectDown, _, cx| {
            highlight(&lowered, down_choices.as_deref(), true, cx);
        })
}

fn list(
    state: &Entity<State>,
    items: Vec<Item>,
    on_select: Option<&Select>,
    cx: &App,
) -> Stateful<Div> {
    let size = if items.iter().any(|item| item.icon.is_some()) {
        RowSize {
            height: ICON_ROW_HEIGHT,
            line: ICON_ROW_LINE,
        }
    } else {
        RowSize {
            height: ROW_HEIGHT,
            line: ROW_LINE,
        }
    };
    let highlighted = state.read(cx).highlighted;
    let rows = items.into_iter().enumerate().flat_map(|(index, item)| {
        let rule = item.divided.then(|| divider(cx).into_any_element());
        let current = highlighted == Some(index);
        let entry = row(state, index, item, current, size, on_select, cx);
        rule.into_iter().chain([entry.into_any_element()])
    });
    div().id("dropdown-items").flex().flex_col().children(rows)
}

fn row(
    state: &Entity<State>,
    index: usize,
    item: Item,
    current: bool,
    size: RowSize,
    on_select: Option<&Select>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let picked = state.clone();
    let hovered = state.clone();
    let on_select = on_select.cloned();
    let color = match (item.danger, current) {
        (true, _) => theme.danger,
        (false, true) => theme.group_box_foreground,
        (false, false) => theme.foreground,
    };
    let shortcut = item.shortcut.map(|key| shortcut_chips(key, cx));
    div()
        .id(index)
        .h(px(size.height))
        .px(px(ROW_PADDING))
        .flex()
        .items_center()
        .gap(px(ROW_ICON_GAP))
        .text_size(px(ROW_TEXT))
        .line_height(px(size.line))
        .text_color(color)
        .when(current, |this| this.bg(theme.secondary))
        .when(item.disabled, |this| this.opacity(DISABLED_OPACITY))
        .when(!item.disabled, |this| {
            this.cursor_pointer()
                .on_mouse_move(move |_, _, cx| {
                    hovered.update(cx, |state, cx| {
                        if state.highlighted != Some(index) {
                            state.highlighted = Some(index);
                            cx.notify();
                        }
                    });
                })
                .on_click(move |_, window, cx| {
                    picked.update(cx, State::close);
                    if let Some(on_select) = &on_select {
                        on_select(index, window, cx);
                    }
                })
        })
        .when_some(item.icon, |this, icon| {
            this.child(Icon::new(icon).size(px(ROW_ICON)).text_color(color))
        })
        .child(
            div()
                .flex_auto()
                .min_w_0()
                .truncate()
                .when(shortcut.is_some(), |this| {
                    this.pr(px(SHORTCUT_GAP - ROW_ICON_GAP))
                })
                .child(item.label),
        )
        .when_some(item.detail, |this, detail| {
            this.child(
                div()
                    .flex_none()
                    .text_size(px(DETAIL_TEXT))
                    .text_color(theme.muted_foreground)
                    .child(detail),
            )
        })
        .children(shortcut)
}

fn shortcut_chips(key: u8, cx: &App) -> Div {
    let color = theme::keycaps().legend;
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(panel::KBD_GAP))
        .children(
            panel::shortcut(key, false)
                .into_iter()
                .map(|legend| panel::kbd(legend, color, cx)),
        )
}

fn divider(cx: &App) -> Div {
    div()
        .flex_none()
        .w_full()
        .h(px(DIVIDER))
        .bg(cx.theme().border)
        .opacity(DIVIDER_OPACITY)
}

fn press(
    state: &Entity<State>,
    event: &KeyDownEvent,
    choices: Option<&Choices>,
    window: &mut Window,
    cx: &mut App,
) {
    match event.keystroke.key.as_str() {
        "escape" => dismiss(state, cx),
        "down" => highlight(state, choices, true, cx),
        "up" => highlight(state, choices, false, cx),
        "enter" | "space" => choose(state, choices, window, cx),
        _ => {}
    }
}

fn dismiss(state: &Entity<State>, cx: &mut App) {
    state.update(cx, State::close);
    cx.stop_propagation();
}

fn highlight(state: &Entity<State>, choices: Option<&Choices>, forward: bool, cx: &mut App) {
    let Some(choices) = choices else {
        return;
    };
    let next = choices.step(state.read(cx).highlighted, forward);
    state.update(cx, |state, cx| {
        state.highlighted = next;
        cx.notify();
    });
    cx.stop_propagation();
}

fn choose(state: &Entity<State>, choices: Option<&Choices>, window: &mut Window, cx: &mut App) {
    let Some(choices) = choices else {
        return;
    };
    cx.stop_propagation();
    let Some(index) = state
        .read(cx)
        .highlighted
        .filter(|index| choices.is_enabled(*index))
    else {
        return;
    };
    state.update(cx, State::close);
    if let Some(on_select) = &choices.on_select {
        on_select(index, window, cx);
    }
}

impl Choices {
    fn is_enabled(&self, index: usize) -> bool {
        self.enabled.get(index).copied().unwrap_or(false)
    }

    fn step(&self, from: Option<usize>, forward: bool) -> Option<usize> {
        let count = self.enabled.len();
        let mut index = from;
        for _ in 0..count {
            let next = match index {
                None if forward => 0,
                None => count - 1,
                Some(current) if forward => (current + 1) % count,
                Some(current) => (current + count - 1) % count,
            };
            if self.is_enabled(next) {
                return Some(next);
            }
            index = Some(next);
        }
        from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_enabled(enabled: &[bool]) -> Choices {
        Choices {
            enabled: enabled.to_vec(),
            on_select: None,
        }
    }

    #[test]
    fn arrow_keys_wrap_and_skip_disabled_items() {
        let choices = with_enabled(&[true, false, true]);
        assert_eq!(choices.step(None, true), Some(0));
        assert_eq!(choices.step(Some(0), true), Some(2));
        assert_eq!(choices.step(Some(2), true), Some(0));
        assert_eq!(choices.step(Some(0), false), Some(2));
        assert_eq!(choices.step(None, false), Some(2));
    }

    #[test]
    fn arrow_keys_keep_the_highlight_when_nothing_is_enabled() {
        let choices = with_enabled(&[false, false]);
        assert_eq!(choices.step(Some(1), true), Some(1));
        assert_eq!(with_enabled(&[]).step(None, true), None);
    }
}
