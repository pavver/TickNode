//! Variables tab in the left panel.

use crate::state::use_editor;
use crate::variables_panel::VariablesList;
use leptos::prelude::*;

#[component]
pub fn VariablesPanel() -> impl IntoView {
    let ed = use_editor();
    view! {
        <VariablesList sheet=ed.open_sheet.get() />
    }
}
