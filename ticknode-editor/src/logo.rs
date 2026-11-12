//! The TickNode mark in the header. It plays the signal animation while the editor connects and
//! while the engine runs nodes; a cycle in progress always finishes, so it never stops mid-flash.

use crate::state::{ConnStatus, use_editor};
use leptos::prelude::*;
use ticknode_shared::SheetSummary;

/// One animation cycle (`logo-*` keyframes in `styles/logo.css`).
const CYCLE_MS: f64 = 4000.0;

/// The mark from `assets/logo/ticknode.svg`, plus the animated layers. The 1 px outline uses
/// `currentColor`, i.e. the theme's text color.
const MARK: &str = r##"<defs>
<clipPath id="logo-wire"><path d="M16 36H20V0H44V48H48V56H36V8H28V44H16Z"/></clipPath>
<clipPath id="logo-src"><path d="M0 28h16v24H0Z"/></clipPath>
<clipPath id="logo-dst"><path d="M48 40h16v24H48Z"/></clipPath>
</defs>
<path d="M16 36H20V0H44V48H48V56H36V8H28V44H16Z" fill="#1E7F88"/>
<path class="logo-run" d="M16 40H24V4H40V52H48" pathLength="100" fill="none" stroke="#00B7C2" stroke-width="8" clip-path="url(#logo-wire)"/>
<rect x="4" y="32" width="8" height="16" fill="#3B4859"/>
<path class="logo-src-idle" d="M0 28h16v24H0Z M4 32v16h8V32Z" fill="#FFB020"/>
<path class="logo-src-base" d="M0 28h16v24H0Z M4 32v16h8V32Z" fill="#E6EDF3"/>
<path class="logo-src" d="M0 28h16v24H0Z M4 32v16h8V32Z" fill="#FFB020"/>
<rect x="52" y="44" width="8" height="16" fill="#2C3746"/>
<path d="M48 40h16v24H48Z M52 44v16h8V44Z" fill="#E6EDF3"/>
<path class="logo-dst" d="M48 40h16v24H48Z M52 44v16h8V44Z" fill="#FFB020"/>
<g fill="none" stroke="currentColor" stroke-width="2">
<path d="M16 36H20V0H44V48H48V56H36V8H28V44H16Z" clip-path="url(#logo-wire)" vector-effect="non-scaling-stroke"/>
<path d="M0 28h16v24H0Z" clip-path="url(#logo-src)" vector-effect="non-scaling-stroke"/>
<path d="M48 40h16v24H48Z" clip-path="url(#logo-dst)" vector-effect="non-scaling-stroke"/>
</g>"##;

/// Whether the mark should be animating at `now`.
pub fn logo_active(status: ConnStatus, activity_at: f64, now: f64) -> bool {
    status == ConnStatus::Connecting || now - activity_at < CYCLE_MS
}

/// When the most recent run reported by sheet summaries happened, if it is recent enough to count.
/// Summaries carry each sheet's run age at the moment they were sent.
pub fn summary_activity(sheets: &[SheetSummary], received_at: f64) -> Option<f64> {
    let age = sheets.iter().filter_map(|s| s.last_executed_ms).min()? as f64;
    (age < CYCLE_MS).then(|| received_at - age)
}

#[component]
pub fn Logo() -> impl IntoView {
    let ed = use_editor();
    let active =
        Memo::new(move |_| logo_active(ed.status.get(), ed.activity_at.get(), ed.now.get()));
    let playing = RwSignal::new(false);
    Effect::new(move |_| {
        if active.get() {
            playing.set(true)
        }
    });
    view! {
        <svg
            class=move || if playing.get() { "logo playing" } else { "logo" }
            viewBox="0 0 64 64"
            aria-hidden="true"
            inner_html=MARK
            // Stop only between cycles, where every animated layer is invisible.
            on:animationiteration=move |_| if !active.get_untracked() { playing.set(false) }
        ></svg>
    }
}

#[cfg(test)]
mod tests;
