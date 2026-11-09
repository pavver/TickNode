//! Home page: scenario sheets, plugins and system log.

pub mod logic;
mod sheets;

use leptos::prelude::*;
use crate::i18n::t;
use crate::plugins_panel::PluginCard;
use crate::state::use_editor;
use crate::ui::{IconButton, Terminal, icons};
use sheets::SheetsSection;

const HOME_SECTION_KEY: &str = "ticknode.home-section";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeSection {
    #[default]
    Sheets,
    Plugins,
    Log,
}

fn load_home_section() -> HomeSection {
    match window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(HOME_SECTION_KEY).ok().flatten())
        .as_deref()
    {
        Some("plugins") => HomeSection::Plugins,
        Some("log") => HomeSection::Log,
        _ => HomeSection::Sheets,
    }
}

fn save_home_section(sec: HomeSection) {
    if let Some(storage) = window().local_storage().ok().flatten() {
        let val = match sec {
            HomeSection::Sheets => "sheets",
            HomeSection::Plugins => "plugins",
            HomeSection::Log => "log",
        };
        let _ = storage.set_item(HOME_SECTION_KEY, val);
    }
}

#[component]
pub fn HomePage() -> impl IntoView {
    let ed = use_editor();
    let section = RwSignal::new(load_home_section());

    let select_section = move |s: HomeSection| {
        section.set(s);
        save_home_section(s);
    };

    view! {
        <div class="home-page" class:narrow=move || ed.narrow.get()>
            <nav class="home-nav">
                <button
                    class="home-nav-item ctl"
                    class:st-selected=move || section.get() == HomeSection::Sheets
                    title=move || t!("home-sheets")
                    on:click=move |_| select_section(HomeSection::Sheets)
                >
                    <span class="mdi mdi-file-document-multiple-outline" aria-hidden="true"></span>
                    <span>{move || t!("home-sheets")}</span>
                </button>
                <button
                    class="home-nav-item ctl"
                    class:st-selected=move || section.get() == HomeSection::Plugins
                    title=move || t!("plugins-title")
                    on:click=move |_| select_section(HomeSection::Plugins)
                >
                    <span class="mdi mdi-puzzle-outline" aria-hidden="true"></span>
                    <span>{move || t!("plugins-title")}</span>
                </button>
                <button
                    class="home-nav-item ctl"
                    class:st-selected=move || section.get() == HomeSection::Log
                    title=move || t!("home-log")
                    on:click=move |_| select_section(HomeSection::Log)
                >
                    <span class="mdi mdi-console" aria-hidden="true"></span>
                    <span>{move || t!("home-log")}</span>
                </button>
            </nav>
            <div class="home-content">
                {move || match section.get() {
                    HomeSection::Sheets => view! { <SheetsSection /> }.into_any(),
                    HomeSection::Plugins => view! { <HomePlugins /> }.into_any(),
                    HomeSection::Log => view! { <HomeLog /> }.into_any(),
                }}
            </div>
        </div>
    }
}

#[component]
fn HomePlugins() -> impl IntoView {
    let ed = use_editor();
    let names = Memo::new(move |_| ed.plugins.with(|p| p.iter().map(|i| i.name.clone()).collect::<Vec<_>>()));
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
                <IconButton
                    icon=icons::BROOM
                    title=Signal::derive(move || t!("console-clear"))
                    on_click=Callback::new(move |_| ed.logs.set(Vec::new()))
                />
            </div>
            <Terminal lines=ed.logs />
        </div>
    }
}
