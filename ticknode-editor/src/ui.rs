//! Operator UI components (NotifiApp design system), ported from Kotlin/Compose to Leptos.

use std::time::Duration;

use leptos::html;
use leptos::prelude::*;

use crate::i18n::t;
use crate::state::{LogKind, LogLine};

/// Duration of the active Success/Error phase (followed by a 500 ms border fade-out in CSS).
const RESULT_FLASH: Duration = Duration::from_millis(800);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlState {
    #[default]
    Idle,
    Pending,
    Success,
    Error,
}

impl ControlState {
    pub fn class(self) -> &'static str {
        match self {
            Self::Idle => "st-idle",
            Self::Pending => "st-pending",
            Self::Success => "st-success",
            Self::Error => "st-error",
        }
    }
}

/// Transitions state to Success/Error and resets it to Idle after the active flash duration.
pub fn flash(state: RwSignal<ControlState>, result: ControlState) {
    state.set(result);
    set_timeout(
        move || {
            if state.try_get_untracked() == Some(result) {
                state.set(ControlState::Idle);
            }
        },
        RESULT_FLASH,
    );
}

pub mod icons {
    // Material glyphs in a 24x24 viewBox (matching OperatorIcons.kt).
    pub const ADD: &str = "M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z";
    pub const DELETE: &str = "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z";
    pub const CLOSE: &str = "M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z";
    pub const CHECK: &str = "M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z";
    pub const SEARCH: &str = "M15.5 14h-.79l-.28-.27A6.47 6.47 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z";
    pub const COPY: &str = "M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z";
    pub const DUPLICATE: &str = "M4 6H2v14c0 1.1.9 2 2 2h14v-2H4V6zm16-4H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-1 9h-4v4h-2v-4H9V9h4V5h2v4h4v2z";
    pub const PLAY: &str = "M8 5v14l11-7z";
    pub const TERMINAL: &str = "M3 13h2v-2H3v2zm0 4h2v-2H3v2zm0-8h2V7H3v2zm4 4h14v-2H7v2zm0 4h14v-2H7v2zM7 7v2h14V7H7z";
    pub const FIT: &str = "M5 15H3v4c0 1.1.9 2 2 2h4v-2H5v-4zM5 5h4V3H5c-1.1 0-2 .9-2 2v4h2V5zm14-2h-4v2h4v4h2V5c0-1.1-.9-2-2-2zm0 16h-4v2h4c1.1 0 2-.9 2-2v-4h-2v4z";
    pub const THEME: &str = "M20 8.69V4h-4.69L12 .69 8.69 4H4v4.69L.69 12 4 15.31V20h4.69L12 23.31 15.31 20H20v-4.69L23.31 12 20 8.69zM12 18c-.89 0-1.74-.2-2.5-.55C11.56 16.5 13 14.42 13 12s-1.44-4.5-3.5-5.45C10.26 6.2 11.11 6 12 6c3.31 0 6 2.69 6 6s-2.69 6-6 6z";
    pub const INFO: &str = "M11 7h2v2h-2zm0 4h2v6h-2zm1-9C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8z";
    // Node categories
    pub const BOLT: &str = "M11 21h-1l1-7H7.5c-.58 0-.57-.32-.38-.66.19-.34.05-.08.07-.12C8.48 10.94 10.42 7.54 13 3h1l-1 7h3.5c.49 0 .56.33.47.51l-.07.15C12.96 17.55 11 21 11 21z";
    pub const CALL_SPLIT: &str = "M14 4l2.29 2.29-2.88 2.88 1.42 1.42 2.88-2.88L20 10V4zm-4 0H4v6l2.29-2.29 4.71 4.7V20h2v-8.41l-5.29-5.3z";
    pub const SCHEDULE: &str = "M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm.5-13H11v6l5.25 3.15.75-1.23-4.5-2.67z";
    pub const SEND: &str = "M2.01 21 23 12 2.01 3 2 10l15 2-15 2z";
    pub const STORAGE: &str = "M2 20h20v-4H2v4zm2-3h2v2H4v-2zM2 4v4h20V4H2zm4 3H4V5h2v2zm-4 7h20v-4H2v4zm2-3h2v2H4v-2z";
    pub const CALCULATE: &str = "M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-5.97 4.06L14.09 6l1.41 1.41L16.91 6l1.06 1.06-1.41 1.41 1.41 1.41-1.06 1.06-1.41-1.4-1.41 1.41-1.06-1.06 1.41-1.41-1.41-1.42zm-6.78.66h5v1.5h-5v-1.5zM11.5 16h-2v2H8v-2H6v-1.5h2v-2h1.5v2h2V16zm6.5 1.25h-5v-1.5h5v1.5zm0-2.5h-5v-1.5h5v1.5z";
    pub const ACCOUNT_TREE: &str = "M22 11V3h-7v3H9V3H2v8h7V8h2v10h4v3h7v-8h-7v3h-2V8h2v3z";
    pub const TEXT_FIELDS: &str = "M2.5 4v3h5v12h3V7h5V4h-13zm19 5h-9v3h3v7h3v-7h3V9z";
    pub const LIST: &str = "M4 10.5c-.83 0-1.5.67-1.5 1.5s.67 1.5 1.5 1.5 1.5-.67 1.5-1.5-.67-1.5-1.5-1.5zm0-6c-.83 0-1.5.67-1.5 1.5S3.17 7.5 4 7.5 5.5 6.83 5.5 6 4.83 4.5 4 4.5zm0 12c-.83 0-1.5.68-1.5 1.5s.68 1.5 1.5 1.5 1.5-.68 1.5-1.5-.67-1.5-1.5-1.5zM7 19h14v-2H7v2zm0-6h14v-2H7v2zm0-8v2h14V5H7z";
    pub const DATA_OBJECT: &str = "M4 7v2c0 .55-.45 1-1 1H2v4h1c.55 0 1 .45 1 1v2c0 1.65 1.35 3 3 3h3v-2H7c-.55 0-1-.45-1-1v-2c0-1.3-.84-2.42-2-2.83v-.34C5.16 11.42 6 10.3 6 9V7c0-.55.45-1 1-1h3V4H7C5.35 4 4 5.35 4 7zm17 3c-.55 0-1-.45-1-1V7c0-1.65-1.35-3-3-3h-3v2h3c.55 0 1 .45 1 1v2c0 1.3.84 2.42 2 2.83v.34c-1.16.41-2 1.52-2 2.83v2c0 .55-.45 1-1 1h-3v2h3c1.65 0 3-1.35 3-3v-2c0-.55.45-1 1-1h1v-4h-1z";
    pub const CLOUD: &str = "M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96z";
    pub const BUG: &str = "M20 8h-2.81c-.45-.78-1.07-1.45-1.82-1.96L17 4.41 15.59 3l-2.17 2.17C12.96 5.06 12.49 5 12 5c-.49 0-.96.06-1.41.17L8.41 3 7 4.41l1.62 1.63C7.88 6.55 7.26 7.22 6.81 8H4v2h2.09c-.05.33-.09.66-.09 1v1H4v2h2v1c0 .34.04.67.09 1H4v2h2.81c1.04 1.79 2.97 3 5.19 3s4.15-1.21 5.19-3H20v-2h-2.09c.05-.33.09-.66.09-1v-1h2v-2h-2v-1c0-.34-.04-.67-.09-1H20V8zm-6 8h-4v-2h4v2zm0-4h-4v-2h4v2z";
    pub const SCIENCE: &str = "M19.8 18.4 14 10.67V6.5l1.35-1.69c.26-.33.03-.81-.39-.81H9.04c-.42 0-.65.48-.39.81L10 6.5v4.17L4.2 18.4c-.49.66-.02 1.6.8 1.6h14c.82 0 1.29-.94.8-1.6z";
    pub const EXTENSION: &str = "M20.5 11H19V7c0-1.1-.9-2-2-2h-4V3.5C13 2.12 11.88 1 10.5 1S8 2.12 8 3.5V5H4c-1.1 0-1.99.9-1.99 2v3.8H3.5c1.49 0 2.7 1.21 2.7 2.7s-1.21 2.7-2.7 2.7H2V20c0 1.1.9 2 2 2h3.8v-1.5c0-1.49 1.21-2.7 2.7-2.7 1.49 0 2.7 1.21 2.7 2.7V22H17c1.1 0 2-.9 2-2v-4h1.5c1.38 0 2.5-1.12 2.5-2.5S21.88 11 20.5 11z";
    pub const BROOM: &str = "M16 11h-1V3c0-1.1-.9-2-2-2h-2c-1.1 0-2 .9-2 2v8H8c-2.76 0-5 2.24-5 5v7h18v-7c0-2.76-2.24-5-5-5zm3 10h-2v-3c0-.55-.45-1-1-1s-1 .45-1 1v3h-2v-3c0-.55-.45-1-1-1s-1 .45-1 1v3H9v-3c0-.55-.45-1-1-1s-1 .45-1 1v3H5v-5c0-1.65 1.35-3 3-3h8c1.65 0 3 1.35 3 3v5z";
}

#[component]
pub fn Icon(path: &'static str) -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d=path /></svg>
    }
}

/// Animated border segment for Pending (25% of the perimeter, clockwise).
#[component]
fn PendingRun(state: Signal<ControlState>) -> impl IntoView {
    move || {
        (state.get() == ControlState::Pending).then(|| {
            view! {
                <svg class="sb-run" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
                    <rect x="0" y="0" width="100" height="100" pathLength="100" vector-effect="non-scaling-stroke" />
                </svg>
            }
        })
    }
}

/// Effective state: external state (when non-Idle) takes precedence over local state.
fn effective(external: Option<Signal<ControlState>>, local: RwSignal<ControlState>) -> Signal<ControlState> {
    Signal::derive(move || match external.map(|s| s.get()) {
        Some(s) if s != ControlState::Idle => s,
        _ => local.get(),
    })
}

/// OperatorActionButton: rectangular stateful button; width remains constant during Pending/flash.
#[component]
pub fn ActionButton(
    #[prop(into)] text: Signal<String>,
    on_click: Callback<()>,
    #[prop(optional)] icon: Option<&'static str>,
    #[prop(optional)] danger: bool,
    #[prop(optional)] full: bool,
    #[prop(optional, into)] state: Option<Signal<ControlState>>,
    /// Action without server confirmation: immediately flash Success.
    #[prop(optional)] flash_on_click: bool,
) -> impl IntoView {
    let local = RwSignal::new(ControlState::Idle);
    let state = effective(state, local);
    let click = move |_| {
        if state.get_untracked() == ControlState::Pending {
            return;
        }
        on_click.run(());
        if flash_on_click {
            flash(local, ControlState::Success);
        }
    };
    view! {
        <button
            // Single-string class attribute: separate `class:` bindings would be wiped on state change
            class=move || format!(
                "btn ctl {}{}{}",
                state.get().class(),
                if danger { " danger" } else { "" },
                if full { " full" } else { "" },
            )
            on:click=click
        >
            <span class="btn-content">{icon.map(|p| view! { <Icon path=p /> })}<span>{move || text.get()}</span></span>
            <span class="btn-overlay">
                {move || match state.get() {
                    ControlState::Pending => Some(view! { <span class="spinner"></span> }.into_any()),
                    ControlState::Success => Some(view! { <Icon path=icons::CHECK /> }.into_any()),
                    ControlState::Error => Some(view! { <Icon path=icons::CLOSE /> }.into_any()),
                    _ => None,
                }}
            </span>
            <PendingRun state=state />
        </button>
    }
}

/// OperatorIconButton: 9 DU square icon button.
#[component]
pub fn IconButton(
    icon: &'static str,
    #[prop(into)] title: Signal<String>,
    on_click: Callback<()>,
    #[prop(optional, into)] selected: Option<Signal<bool>>,
) -> impl IntoView {
    view! {
        <button
            class="icon-btn ctl"
            class:st-selected=move || selected.is_some_and(|s| s.get())
            title=move || title.get()
            aria-label=move || title.get()
            on:click=move |_| on_click.run(())
        >
            <Icon path=icon />
        </button>
    }
}

/// OperatorTextField: text input with a SAVE button that appears when the value is modified.
#[component]
pub fn TextField(
    /// Saved value (from the server).
    #[prop(into)] value: Signal<String>,
    #[prop(optional, into)] placeholder: Signal<String>,
    #[prop(optional, into)] state: Option<Signal<ControlState>>,
    /// Without `on_save`, the field acts as a live search input via `on_input`.
    #[prop(optional)] on_save: Option<Callback<String>>,
    #[prop(optional)] on_input: Option<Callback<String>>,
    #[prop(optional)] icon: Option<&'static str>,
    #[prop(optional)] numeric: bool,
) -> impl IntoView {
    let draft = RwSignal::new(value.get_untracked());
    // A new value from the server replaces the draft
    Effect::new(move |_| draft.set(value.get()));

    let local = RwSignal::new(ControlState::Idle);
    let state = effective(state, local);
    let dirty = move || on_save.is_some() && draft.get() != value.get();
    let save = move || {
        if let Some(cb) = on_save {
            if draft.get_untracked() != value.get_untracked() {
                cb.run(draft.get_untracked());
            }
        }
    };

    view! {
        <div class=move || format!("text-field ctl {}", state.get().class())>
            {icon.map(|p| view! { <span class="icon-slot"><Icon path=p /></span> })}
            <input
                type="text"
                inputmode=if numeric { "decimal" } else { "text" }
                placeholder=move || placeholder.get()
                prop:value=move || draft.get()
                on:input=move |ev| {
                    let v = event_target_value(&ev);
                    draft.set(v.clone());
                    if let Some(cb) = on_input {
                        cb.run(v);
                    }
                }
                on:keydown=move |ev| {
                    if ev.key() == "Enter" {
                        save();
                    } else if ev.key() == "Escape" {
                        draft.set(value.get_untracked());
                    }
                }
            />
            {move || dirty().then(|| view! { <button class="save-btn" on:click=move |_| save()>{move || t!("ui-save")}</button> })}
            <PendingRun state=state />
        </div>
    }
}

/// OperatorAsyncSwitch: thumb position changes only after confirmation (server value update).
#[component]
pub fn Switch(
    #[prop(into)] checked: Signal<bool>,
    on_toggle: Callback<bool>,
    #[prop(optional, into)] state: Option<Signal<ControlState>>,
    /// Compact variant (used inline on a node).
    #[prop(optional)] small: bool,
) -> impl IntoView {
    let local = RwSignal::new(ControlState::Idle);
    let state = effective(state, local);
    view! {
        <button
            // Single-string class attribute: a separate `class:on` binding would be wiped on state
            // change (Pending -> Success), leaving the switch visually off
            class=move || format!(
                "switch {}{}{}",
                state.get().class(),
                if checked.get() { " on" } else { "" },
                if small { " small" } else { "" },
            )
            role="switch"
            aria-checked=move || checked.get().to_string()
            on:click=move |_| on_toggle.run(!checked.get_untracked())
        >
            <span class="thumb"></span>
        </button>
    }
}

#[component]
pub fn StatusDot(#[prop(into)] class: Signal<&'static str>) -> impl IntoView {
    view! { <span class=move || format!("status-dot {}", class.get())></span> }
}

/// OperatorConsoleTerminal with auto-scroll when the user is already near the bottom.
#[component]
pub fn Terminal(#[prop(into)] lines: Signal<Vec<LogLine>>) -> impl IntoView {
    let el = NodeRef::<html::Div>::new();
    Effect::new(move |_| {
        lines.track();
        if let Some(div) = el.get_untracked() {
            let near_bottom = div.scroll_height() - div.scroll_top() - div.client_height() < 48;
            if near_bottom {
                request_animation_frame(move || div.set_scroll_top(div.scroll_height()));
            }
        }
    });
    view! {
        <div class="terminal" node_ref=el>
            <For each=move || lines.get() key=|l| l.id let:line>
                <div class=format!("terminal-line {}", match line.kind {
                    LogKind::Error => "error",
                    LogKind::System => "system",
                    LogKind::Node => "",
                })>
                    <span class="ts">{line.time}" "</span>{line.text}
                </div>
            </For>
        </div>
    }
}
