//! Node help: an info icon whose hover card explains the node and shows its state.

use leptos::prelude::*;
use operator_ui::prelude::HoverCard;
use ticknode_shared::*;

use crate::categories::category_label;
use crate::i18n::{self, t};
use crate::ui::{Icon, icons};

/// Width of the help card in DU.
const HELP_WIDTH_DU: f32 = 90.0;

/// Info icon with a hover card: title, category, type and cost, description and help; for a node on
/// the canvas (`id`) also its frozen pins and its last execution.
#[component]
pub fn NodeHelp(
    #[prop(into)] def: Signal<NodeDefinition>,
    #[prop(optional)] id: Option<NodeId>,
) -> impl IntoView {
    let card = move || {
        let help = def.with(|d| {
            let help = i18n::node_help(d);
            (help != i18n::node_description(d)).then_some(help)
        });
        view! {
            <div class="node-help">
                <div class="op-t-row-title">{move || def.with(i18n::node_title)}</div>
                <div class="op-t-caption node-help-meta" title=move || t!("inspector-cost-help")>
                    {move || def.with(|d| format!("{} · {} · {}", category_label(&d.category), d.type_name, t!("inspector-cost", cost = d.cost)))}
                </div>
                <p class="node-help-text">{move || def.with(i18n::node_description)}</p>
                {help.map(|h| view! { <p class="node-help-text">{h}</p> })}
                {id.map(|id| view! {
                    <crate::frozen::FrozenInfo id=id />
                    <super::execution::ExecutionInfo id=id />
                })}
            </div>
        }
    };
    view! {
        <span
            class="info-btn"
            // Neither drag the node or palette block nor select it
            on:pointerdown=|ev: web_sys::PointerEvent| ev.stop_propagation()
            on:click=|ev: web_sys::MouseEvent| ev.stop_propagation()
        >
            <HoverCard card=ViewFn::from(card) width=HELP_WIDTH_DU label=t!("palette-info-aria")>
                <span class="info-icon" role="button" tabindex="0" aria-label=move || t!("palette-info-aria")>
                    <Icon path=icons::INFO />
                </span>
            </HoverCard>
        </span>
    }
}
