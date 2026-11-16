//! Translated node, property, category, type and setting definitions.

use super::{TYPES, lookup, slug};
use ticknode_shared::*;

fn node_slug(type_name: &str) -> String {
    slug(type_name)
}

/// Translated title of a data type; `None` if the server shipped no translation for it.
fn translated_type_title(type_name: &str) -> Option<String> {
    lookup(&format!("type-{}-title", slug(type_name)))
}

pub fn node_title(def: &NodeDefinition) -> String {
    lookup(&format!("node-{}-title", node_slug(&def.type_name)))
        .unwrap_or_else(|| def.title.clone())
}

pub fn node_description(def: &NodeDefinition) -> String {
    lookup(&format!("node-{}-description", node_slug(&def.type_name)))
        .unwrap_or_else(|| def.description.clone())
}

pub fn node_help(def: &NodeDefinition) -> String {
    lookup(&format!("node-{}-help", node_slug(&def.type_name))).unwrap_or_else(|| {
        if def.help.trim().is_empty() {
            node_description(def)
        } else {
            def.help.clone()
        }
    })
}

/// Pin label. Convention: `node-<type slug>-pin-<name>` where the name is the pin's name as written when it
/// consists of `[A-Za-z0-9_-]`, otherwise its slug (`Delay ms` -> `delay-ms`).
pub fn node_pin_label(type_name: &str, pin_name: &str) -> String {
    let base = format!("node-{}-pin-", node_slug(type_name));
    if !pin_name.is_empty()
        && pin_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && let Some(label) = lookup(&format!("{base}{pin_name}"))
    {
        return label;
    }
    let pin_slug = slug(pin_name);
    if !pin_slug.is_empty()
        && let Some(label) = lookup(&format!("{base}{pin_slug}"))
    {
        return label;
    }
    pin_name.to_string()
}

/// Universal object nodes get their field labels from the selected struct's translations.
pub fn node_pin_label_with_properties(
    type_name: &str,
    properties: &[(String, DataValue)],
    pin: PinId,
    pin_name: &str,
) -> String {
    if pin != PinId(0)
        && matches!(type_name, "object/make" | "object/break")
        && let Some(label) = object_field_label(properties, pin_name)
    {
        return label;
    }
    node_pin_label(type_name, pin_name)
}

fn object_field_label(properties: &[(String, DataValue)], pin_name: &str) -> Option<String> {
    let (_, DataValue::String(type_name)) = properties.iter().find(|(name, _)| name == "type")?
    else {
        return None;
    };
    let field_name = TYPES.with(|types| {
        let types = types.borrow();
        let field = types.get(type_name)?.fields().iter().find(|field| {
            field.title == pin_name || (field.title.is_empty() && field.name == pin_name)
        })?;
        Some(field.name.clone())
    })?;
    lookup(&format!("type-{}-field-{field_name}", slug(type_name)))
}

/// Fluent key fragment of a property name: the name as written when it consists of `[A-Za-z0-9_-]`
/// (`entity_id`), otherwise its slug.
fn property_key(name: &str) -> Vec<String> {
    let mut keys = Vec::new();
    if !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        keys.push(name.to_string());
    }
    let name_slug = slug(name);
    if !name_slug.is_empty() && !keys.contains(&name_slug) {
        keys.push(name_slug);
    }
    keys
}

fn property_lookup(type_name: &str, name: &str, suffix: &str) -> Option<String> {
    let base = format!("node-{}-prop-", node_slug(type_name));
    property_key(name)
        .iter()
        .find_map(|key| lookup(&format!("{base}{key}-{suffix}")))
}

/// Title of a described property (`node-<type slug>-prop-<name>-title`), falling back to the definition's title.
pub fn property_title(type_name: &str, prop: &PropertyDefinition) -> String {
    property_lookup(type_name, &prop.name, "title").unwrap_or_else(|| prop.title.clone())
}

pub fn property_description(type_name: &str, prop: &PropertyDefinition) -> String {
    property_lookup(type_name, &prop.name, "description")
        .unwrap_or_else(|| prop.description.clone())
}

/// Select labels prefer node-specific keys, then known type titles, unit symbols, and definition text.
pub fn property_option_label(type_name: &str, prop_name: &str, value: &str, label: &str) -> String {
    if prop_name == "locale" && value == "server" {
        return super::engine::server_locale_label();
    }
    property_lookup(type_name, prop_name, &format!("option-{}", slug(value)))
        .or_else(|| {
            // Only named types (`quantity:<name>` targets, `plugin.type` values): plain words such as
            // `bool` keep their own option labels.
            let name = value.strip_prefix("quantity:").unwrap_or(value);
            let known = name.contains('.')
                && (TYPES.with(|types| types.borrow().get(name).is_some())
                    || quantity::kind(name).is_some());
            known
                .then(|| {
                    translated_type_title(name).or_else(|| lookup(&format!("type-{}", slug(name))))
                })
                .flatten()
        })
        .or_else(|| {
            (prop_name == "unit")
                .then(|| lookup(&format!("unit-{}", slug(value))))
                .flatten()
        })
        .unwrap_or_else(|| label.to_string())
}

/// Compact MultiSelect text; the full option label remains available as a tooltip.
pub fn property_option_short_label(
    type_name: &str,
    name: &str,
    value: &str,
    label: &str,
) -> String {
    property_lookup(type_name, name, &format!("option-{}-short", slug(value))).unwrap_or_else(
        || {
            property_option_label(type_name, name, value, label)
                .chars()
                .take(3)
                .collect()
        },
    )
}

/// Whether a named pin type has an expandable struct definition in the current catalog.
pub(crate) fn pin_can_split(pin: &PinDefinition) -> bool {
    TYPES.with(|types| splittable_type(&types.borrow(), pin).is_some())
}

/// Translated category name for categories without a built-in label (`category-<slug>`).
pub fn plugin_category_label(category: &str) -> Option<String> {
    lookup(&format!("category-{}", slug(category)))
}

/// Title of a data type definition.
pub fn type_def_title(def: &TypeDefinition) -> String {
    translated_type_title(&def.name).unwrap_or_else(|| def.title.clone())
}

/// Title of an enum value in the drop-down (the value itself when it has no title).
pub fn enum_value_title(type_name: &str, value: &EnumValue) -> String {
    lookup(&format!(
        "type-{}-value-{}",
        slug(type_name),
        slug(&value.value)
    ))
    .unwrap_or_else(|| {
        if value.title.is_empty() {
            value.value.clone()
        } else {
            value.title.clone()
        }
    })
}

/// Localized selected enum label for content-based geometry; structs have no inline control.
pub(crate) fn enum_selected_label(name: &str, selected: &str) -> Option<String> {
    TYPES.with(|types| {
        let types = types.borrow();
        let def = types.get(name).filter(|d| d.is_enum())?;
        Some(
            def.values()
                .iter()
                .find(|value| value.value == selected)
                .map_or_else(
                    || {
                        if selected.is_empty() {
                            "—".into()
                        } else {
                            selected.into()
                        }
                    },
                    |value| enum_value_title(name, value),
                ),
        )
    })
}

/// Title of a plugin setting (`setting-<plugin slug>-<key slug>-title`).
pub fn setting_title(plugin: &str, def: &SettingDefinition) -> String {
    lookup(&format!(
        "setting-{}-{}-title",
        slug(plugin),
        slug(&def.key)
    ))
    .unwrap_or_else(|| def.title.clone())
}

pub fn setting_description(plugin: &str, def: &SettingDefinition) -> String {
    lookup(&format!(
        "setting-{}-{}-description",
        slug(plugin),
        slug(&def.key)
    ))
    .unwrap_or_else(|| def.description.clone())
}

/// Label of a `Select` setting option.
pub fn setting_option_label(
    plugin: &str,
    def: &SettingDefinition,
    value: &str,
    label: &str,
) -> String {
    lookup(&format!(
        "setting-{}-{}-option-{}",
        slug(plugin),
        slug(&def.key),
        slug(value)
    ))
    .unwrap_or_else(|| label.to_string())
}

/// Title of a plugin resource collection (`<plugin slug>-resource-<collection id slug>-title`).
pub fn resource_collection_title(plugin: &str, collection: &ResourceCollection) -> String {
    lookup(&format!(
        "{}-resource-{}-title",
        slug(plugin),
        slug(&collection.id)
    ))
    .unwrap_or_else(|| collection.title.clone())
}

/// Title of a resource action (`<plugin slug>-resource-<collection id slug>-action-<type_name slug>`).
pub fn resource_action_title(plugin: &str, collection_id: &str, action: &ResourceAction) -> String {
    lookup(&format!(
        "{}-resource-{}-action-{}",
        slug(plugin),
        slug(collection_id),
        slug(&action.type_name)
    ))
    .unwrap_or_else(|| action.title.clone())
}
