mod advanced;
mod diagnostics;
mod firmware;
mod gamepad;
mod lightbar;
mod lighting;
mod performance;
mod profiles;
mod quick;
mod remap;
mod settings;
mod switches;

use gpui_kit::{
    AnyView, AppContext as _, Entity, Window, assets::IconName as Catalog, component::Icon,
};
pub(crate) use profiles::{ProfilesPage, avatar as profile_avatar};

use crate::store::{Keyboard, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Page {
    Quick,
    Actuation,
    RapidTrigger,
    Remap,
    Advanced,
    Gamepad,
    Lighting,
    Lightbar,
    Profiles,
    Settings,
    Switches,
    Firmware,
    Diagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Group {
    Keyboard,
    Device,
}

impl Page {
    pub(crate) const ALL: [Self; 13] = [
        Self::Quick,
        Self::Actuation,
        Self::RapidTrigger,
        Self::Remap,
        Self::Advanced,
        Self::Gamepad,
        Self::Lighting,
        Self::Lightbar,
        Self::Profiles,
        Self::Settings,
        Self::Switches,
        Self::Firmware,
        Self::Diagnostics,
    ];

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Quick => "Quick settings",
            Self::Actuation => "Actuation point",
            Self::RapidTrigger => "Rapid Trigger",
            Self::Remap => "Remap",
            Self::Advanced => "Advanced keys",
            Self::Gamepad => "Gamepad",
            Self::Lighting => "Lighting",
            Self::Lightbar => "Light bar",
            Self::Profiles => "My profiles",
            Self::Settings => "Keyboard settings",
            Self::Switches => "Switch selector",
            Self::Firmware => "Firmware",
            Self::Diagnostics => "Diagnostics",
        }
    }

    pub(crate) fn icon(self) -> Icon {
        Icon::new(match self {
            Self::Quick => Catalog::SlidersHorizontal,
            Self::Actuation => Catalog::ArrowUpDown,
            Self::RapidTrigger => Catalog::Zap,
            Self::Remap => Catalog::Keyboard,
            Self::Advanced => Catalog::Layers,
            Self::Gamepad => Catalog::Gamepad2,
            Self::Lighting => Catalog::Palette,
            Self::Lightbar => Catalog::Lightbulb,
            Self::Profiles => Catalog::UserRound,
            Self::Settings => Catalog::Settings,
            Self::Switches => Catalog::ToggleRight,
            Self::Firmware => Catalog::Cpu,
            Self::Diagnostics => Catalog::Activity,
        })
    }

    pub(crate) const fn group(self) -> Group {
        match self {
            Self::Quick
            | Self::Actuation
            | Self::RapidTrigger
            | Self::Remap
            | Self::Advanced
            | Self::Gamepad
            | Self::Lighting
            | Self::Lightbar
            | Self::Profiles => Group::Keyboard,
            Self::Settings | Self::Switches | Self::Firmware | Self::Diagnostics => Group::Device,
        }
    }

    pub(crate) const fn in_configuration(self) -> bool {
        matches!(self.group(), Group::Keyboard) && !matches!(self, Self::Quick | Self::Profiles)
    }

    pub(crate) const fn needs_keyboard(self) -> bool {
        !matches!(self, Self::Firmware)
    }

    pub(crate) fn is_available(self, keyboard: Option<&Keyboard>) -> bool {
        match self {
            Self::Lightbar => {
                keyboard.is_some_and(|keyboard| keyboard.capabilities.lightbar_leds > 0)
            }
            _ => true,
        }
    }

    pub(crate) fn open(
        self,
        store: &Entity<Store>,
        window: &mut Window,
        cx: &mut gpui_kit::App,
    ) -> AnyView {
        let store = store.clone();
        match self {
            Self::Quick => cx.new(|cx| quick::QuickPage::new(store, window, cx)).into(),
            Self::Actuation => cx
                .new(|cx| performance::ActuationPage::new(store, window, cx))
                .into(),
            Self::RapidTrigger => cx
                .new(|cx| performance::RapidTriggerPage::new(store, window, cx))
                .into(),
            Self::Remap => cx.new(|cx| remap::RemapPage::new(store, window, cx)).into(),
            Self::Advanced => cx
                .new(|cx| advanced::AdvancedPage::new(store, window, cx))
                .into(),
            Self::Gamepad => cx
                .new(|cx| gamepad::GamepadPage::new(store, window, cx))
                .into(),
            Self::Lighting => cx
                .new(|cx| lighting::LightingPage::new(store, window, cx))
                .into(),
            Self::Lightbar => cx
                .new(|cx| lightbar::LightbarPage::new(store, window, cx))
                .into(),
            Self::Profiles => cx.new(|cx| ProfilesPage::new(store, window, cx)).into(),
            Self::Settings => cx
                .new(|cx| settings::SettingsPage::new(store, window, cx))
                .into(),
            Self::Switches => cx
                .new(|cx| switches::SwitchesPage::new(store, window, cx))
                .into(),
            Self::Firmware => cx
                .new(|cx| firmware::FirmwarePage::new(store, window, cx))
                .into(),
            Self::Diagnostics => cx
                .new(|cx| diagnostics::DiagnosticsPage::new(store, window, cx))
                .into(),
        }
    }
}
