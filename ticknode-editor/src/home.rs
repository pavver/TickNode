//! Home page: scenario sheets, plugins and system log.

pub mod logic;
mod settings;
mod sheets;
mod usage;

use crate::i18n::t;
use crate::plugins_panel::PluginCard;
use crate::state::use_editor;
use crate::ui::{log_text, op_icons};
use leptos::prelude::*;
use operator_ui::prelude::{ConsoleTerminal, IconButton, NavigationRail, RailItem};
use settings::SettingsSection;
use sheets::SheetsSection;
use usage::UsageSection;

const HOME_SECTION_KEY: &str = "ticknode.home-section";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeSection {
    #[default]
    Sheets,
    Variables,
    Plugins,
    Log,
    Usage,
    Settings,
}

impl HomeSection {
    const ALL: [Self; 6] = [
        Self::Sheets,
        Self::Variables,
        Self::Plugins,
        Self::Log,
        Self::Usage,
        Self::Settings,
    ];

    /// Stable id: the navigation item id and the stored value.
    fn id(self) -> &'static str {
        match self {
            Self::Sheets => "sheets",
            Self::Variables => "variables",
            Self::Plugins => "plugins",
            Self::Log => "log",
            Self::Usage => "usage",
            Self::Settings => "settings",
        }
    }

    fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.id() == id)
            .unwrap_or_default()
    }

    /// Short label that fits the navigation rail.
    fn label(self) -> String {
        match self {
            Self::Sheets => t!("home-rail-sheets"),
            Self::Variables => t!("home-rail-variables"),
            Self::Plugins => t!("home-rail-plugins"),
            Self::Log => t!("home-rail-log"),
            Self::Usage => t!("home-rail-usage"),
            Self::Settings => t!("home-rail-settings"),
        }
    }

    fn icon(self) -> operator_ui::tokens::Icon {
        match self {
            Self::Sheets => rail_icons::SHEETS,
            Self::Variables => rail_icons::VARIABLES,
            Self::Plugins => rail_icons::PLUGINS,
            Self::Log => rail_icons::LOG,
            Self::Usage => rail_icons::USAGE,
            Self::Settings => operator_ui::prelude::icons::SETTINGS,
        }
    }
}

/// Home destinations drawn from the editor's 24-unit Material paths.
mod rail_icons {
    use crate::ui::icons;
    use operator_ui::tokens::Icon;

    const fn material(name: &'static str, paths: &'static [&'static str]) -> Icon {
        Icon {
            name,
            view_box: "0 0 24 24",
            paths,
        }
    }

    pub const SHEETS: Icon = material("account_tree", &[icons::ACCOUNT_TREE]);
    pub const VARIABLES: Icon = material("data_object", &[icons::DATA_OBJECT]);
    pub const PLUGINS: Icon = material("extension", &[icons::EXTENSION]);
    pub const LOG: Icon = material("list", &[icons::TERMINAL]);
    pub const USAGE: Icon = material("schedule", &[icons::SCHEDULE]);
}

fn rail_items() -> Vec<RailItem> {
    HomeSection::ALL
        .into_iter()
        .map(|s| RailItem::new(s.id(), s.label(), s.icon()))
        .collect()
}

fn load_home_section() -> HomeSection {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(HOME_SECTION_KEY).ok().flatten())
        .map_or_else(HomeSection::default, |id| HomeSection::from_id(&id))
}

fn save_home_section(sec: HomeSection) {
    if let Some(storage) = window().local_storage().ok().flatten() {
        let _ = storage.set_item(HOME_SECTION_KEY, sec.id());
    }
}

#[component]
pub fn HomePage() -> impl IntoView {
    let ed = use_editor();
    let section = RwSignal::new(load_home_section());
    let selected = RwSignal::new(section.get_untracked().id().to_string());

    let select_section = move |s: HomeSection| {
        section.set(s);
        save_home_section(s);
    };

    view! {
        <div class="home-page" class:narrow=move || ed.narrow.get()>
            <NavigationRail
                items=Signal::derive(rail_items)
                selected=selected
                on_select=Callback::new(move |id: String| select_section(HomeSection::from_id(&id)))
                horizontal=Signal::derive(move || ed.narrow.get())
                label=t!("home-navigation")
            />
            <div class="home-content">
                {move || match section.get() {
                    HomeSection::Sheets => view! { <SheetsSection /> }.into_any(),
                    HomeSection::Variables => view! { <HomeVariables /> }.into_any(),
                    HomeSection::Plugins => view! { <HomePlugins /> }.into_any(),
                    HomeSection::Log => view! { <HomeLog /> }.into_any(),
                    HomeSection::Usage => view! { <UsageSection /> }.into_any(),
                    HomeSection::Settings => view! { <SettingsSection /> }.into_any(),
                }}
            </div>
        </div>
    }
}

#[component]
fn HomeVariables() -> impl IntoView {
    view! {
        <div class="home-variables-section">
            <crate::variables_panel::VariablesList sheet=None />
        </div>
    }
}

#[component]
fn HomePlugins() -> impl IntoView {
    let ed = use_editor();
    let names = Memo::new(move |_| {
        ed.plugins
            .with(|p| p.iter().map(|i| i.name.clone()).collect::<Vec<_>>())
    });
    view! {
        <div class="home-plugins-section">
            {move || names.with(Vec::is_empty).then(|| view! {
                <div class="empty">{move || t!("plugins-empty")}</div>
            })}
            <For each=move || names.get() key=|n| n.clone() let:name>
                <PluginCard name=name />
            </For>
        </div>
    }
}

#[component]
fn HomeLog() -> impl IntoView {
    let ed = use_editor();
    view! {
        <div class="home-log-section">
            <div class="console-bar">
                <span class="t-section" style="flex:1">{move || t!("home-log")}</span>
                <span class="t-caption">{move || t!("console-lines", count = ed.logs.with(|l| l.len()))}</span>
                {move || view! {
                    <IconButton
                        icon=op_icons::BROOM
                        label=t!("console-clear")
                        size=7.0
                        on_click=Callback::new(move |_| ed.logs.set(Vec::new()))
                    />
                }}
            </div>
            <ConsoleTerminal logs=Signal::derive(move || ed.logs.with(|logs| logs.iter().map(log_text).collect::<Vec<_>>())) />
        </div>
    }
}
