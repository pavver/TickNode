//! Variables tab in the left panel (placeholder).

use leptos::prelude::*;
use crate::i18n::t;

#[component]
pub fn VariablesPanel() -> impl IntoView {
    view! {
        <div class="empty variables-empty">
            {move || t!("variables-coming-soon")}
        </div>
    }
}
