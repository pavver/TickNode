//! Variables tab in the left panel.

use leptos::prelude::*;
use crate::state::use_editor;
use crate::variables_panel::VariablesList;

#[component]
pub fn VariablesPanel() -> impl IntoView {
    let ed = use_editor();
    view! {
        <VariablesList sheet=ed.open_sheet.get() />
    }
}
