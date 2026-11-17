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

/// Width of a status card in DU.
const STATUS_WIDTH_DU: f32 = 70.0;

/// Status icon on a node header (error, frozen pins, missing input) whose hover card explains it.
#[component]
pub fn StatusHint(
    /// Explanation shown in the card; lines become paragraphs.
    #[prop(into)]
    text: Signal<String>,
    /// Class of the trigger, which sets its colour.
    class: &'static str,
    /// Glyph of the trigger: a Material Design Icons class or plain text.
    glyph: &'static str,
) -> impl IntoView {
    let card = move || {
        view! {
            <div class="node-help">
                {move || text.get().lines().map(|line| view! { <p class="node-help-text">{line.to_string()}</p> }).collect_view()}
            </div>
        }
    };
    let icon = if glyph.starts_with("mdi-") {
        view! { <span class=format!("mdi {glyph}") aria-hidden="true"></span> }.into_any()
    } else {
        view! { <span aria-hidden="true">{glyph}</span> }.into_any()
    };
    view! {
        <span
            class=format!("status-hint {class}")
            on:pointerdown=|ev: web_sys::PointerEvent| ev.stop_propagation()
            on:click=|ev: web_sys::MouseEvent| ev.stop_propagation()
        >
            <HoverCard card=ViewFn::from(card) width=STATUS_WIDTH_DU label=text.get_untracked()>
                <span class="info-icon" role="button" tabindex="0" aria-label=move || text.get()>
                    {icon}
                </span>
            </HoverCard>
        </span>
    }
}
