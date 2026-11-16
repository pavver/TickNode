//! Value-aware field hovers share execution's path resolution and on-demand metadata hints.
use crate::state::Editor;
use leptos::prelude::*;
use ticknode_shared::*;

pub(super) fn tooltip(ed: Editor, node: NodeId, path: &[String]) -> Option<String> {
    let parts = if path.first().is_some_and(|p| p == "text") {
        &path[1..]
    } else {
        path
    };
    let full = parts.join(".");
    let inputs = super::preview::node_preview_inputs(ed, node);
    if !inputs
        .iter()
        .any(|(name, _, _)| parts.first() == Some(name))
    {
        ed.request_value_fields(&full, false);
        let tooltip = ed.with_value_tree(|ctx| {
            ed.types.with(|types| {
                let (namespace, entry, suffix) = ctx.catalog.entry(&full)?;
                let ty = ctx.field_type(&full, types)?;
                let value = ctx
                    .value(namespace, &entry.key)
                    .and_then(|v| plugin_values::project(v, suffix));
                let title = crate::i18n::data_type_title(types, &ty);
                let text = super::hover::format_field_tooltip(path, &ty, value, &title);
                let hint = value_tree::type_hint(&ty, types);
                Some(format!("{text}\n{}", crate::i18n::ui_text(&hint)))
            })
        });
        if tooltip.is_some() {
            return tooltip;
        }
    }
    let root = inputs
        .into_iter()
        .filter(|(name, _, _)| full.starts_with(&format!("{name}.")))
        .max_by_key(|(name, _, _)| name.len());
    let (value, declared, suffix) = if let Some((name, value, ty)) = root {
        (
            value,
            ty,
            full.strip_prefix(&name)?
                .trim_start_matches('.')
                .to_string(),
        )
    } else if parts.len() >= 3 && ["var", "sheet", "global"].contains(&parts[0].as_str()) {
        let reference = VariableRef::parse(&format!("{}.{}", parts[0], parts[1]))?;
        let (value, ty) = ed.variables.with(|vars| {
            let var =
                crate::inline::variable::resolve_variable(&reference, ed.open_sheet.get(), vars)?;
            Some((var.value.clone(), var.data_type.clone()))
        })?;
        (Some(value), ty, parts[2..].join("."))
    } else {
        return None;
    };
    ed.types.with(|types| {
        let ty = effective_type(value.as_ref(), &declared, &suffix, types)?;
        let field_value = value
            .as_ref()
            .and_then(|v| plugin_values::project(v, &suffix));
        let (parent_path, name) = suffix.rsplit_once('.').unwrap_or(("", &suffix));
        let parent_ty = effective_type(value.as_ref(), &declared, parent_path, types)?;
        let parent_value = value
            .as_ref()
            .and_then(|v| plugin_values::project(v, parent_path));
        let hint = value_tree::field_hint(parent_value, &parent_ty, name, types);
        let title = crate::i18n::data_type_title(types, &ty);
        let text = super::hover::format_field_tooltip(path, &ty, field_value, &title);
        Some(format!("{text}\n{}", crate::i18n::ui_text(&hint)))
    })
}
