//! Check the actual HA catalog against the canvas font and control budgets in both locales.
use super::*;
use crate::i18n::{self, Locale};
use ha_plugin::HomeAssistant;
use ticknode_plugin_sdk::{NodeId, Plugin, PluginNode};

const ADVANCE: f64 = 1200.0 / 2048.0;
fn text(label: &str, size: f64) -> f64 {
    label.chars().count() as f64 * size * ADVANCE
}

fn check(node: &dyn PluginNode) {
    let props = node.properties();
    let mut def = node.definition();
    // The host projects property-or-input pins before sending the definition to the editor.
    def.pins.retain(|pin| {
        !def.properties.iter().any(|prop| {
            prop.input == Some(pin.id)
                && props.iter().any(|(name, value)| {
                    *name == prop.name && matches!(value, DataValue::String(s) if !s.is_empty())
                })
        })
    });
    let layout = NodeLayout::sized(&def, &props, None, &[]);
    let context = format!("{} / {:?}", def.type_name, i18n::current_locale());
    assert!(
        (NODE_WIDTH..=2.0 * NODE_WIDTH).contains(&layout.width),
        "{context}"
    );
    let title = text(&i18n::node_title(&def), 13.0);
    let badge = text(&crate::categories::category_label(&def.category), 11.0);
    let controls = if def.pins.iter().any(|p| p.pin_type == PinType::Exec) {
        52.0
    } else {
        26.0
    };
    assert!(title + controls <= layout.width, "header title: {context}");
    assert!(
        title + badge + controls <= layout.width || layout.width == 480.0,
        "header badge: {context}"
    );
    for (name, value) in property_rows(&def, &props) {
        let prop = def.properties.iter().find(|p| p.name == *name).unwrap();
        let label = text(&i18n::property_title(&def.type_name, prop), 12.0);
        let field = match &prop.kind {
            PropertyKind::Select(options) => {
                let DataValue::String(selected) = value else {
                    panic!("text select")
                };
                let (v, label) = options.iter().find(|(v, _)| v == selected).unwrap();
                (text(
                    &i18n::property_option_label(&def.type_name, name, v, label),
                    12.0,
                ) + 24.0)
                    .min(160.0)
            }
            PropertyKind::MultiSelect(options) => {
                options
                    .iter()
                    .map(|(v, label)| {
                        let short =
                            i18n::property_option_short_label(&def.type_name, name, v, label);
                        assert!(short.chars().count() <= 3, "chip: {context}");
                        text(&short, 11.0) + 10.0
                    })
                    .sum::<f64>()
                    + 2.0 * options.len().saturating_sub(1) as f64
            }
            PropertyKind::Duration { signed } => {
                // The longest wall-clock unit, 72px number field, gaps and optional sign.
                let unit = crate::inline::duration::units::Unit::ALL
                    .into_iter()
                    .filter(|u| *u != crate::inline::duration::units::Unit::Ticks)
                    .map(|u| text(&u.label(), 12.0))
                    .fold(0.0, f64::max);
                72.0 + unit + 24.0 + 2.0 + if *signed { 22.0 } else { 0.0 }
            }
            _ => 72.0,
        };
        assert!(
            label + field + 28.0 <= layout.width,
            "property {name}: {context}"
        );
    }
    for pin in &def.pins {
        let label = text(&i18n::node_pin_label(&def.type_name, &pin.name), 12.0);
        let split = i18n::pin_can_split(pin);
        // Output flex rows need a socket, gaps, padding and the non-shrinking split button.
        let chrome = if split { 46.0 } else { 24.0 };
        assert!(
            label + chrome <= layout.pin_width(pin.id, pin.direction),
            "pin {}: {context}",
            pin.name
        );
    }
}

#[test]
fn every_ha_node_fits_at_automatic_width_in_both_locales() {
    i18n::set_known_types(&TypeRegistry::from_definitions(HomeAssistant::types()));
    i18n::set_runtime_translations(
        &HomeAssistant::TRANSLATIONS
            .iter()
            .map(|(locale, ftl)| (locale.to_string(), ftl.to_string()))
            .collect::<Vec<_>>(),
    );
    for locale in [Locale::En, Locale::Uk] {
        i18n::with_locale(locale, || {
            for name in HomeAssistant::NODE_TYPES {
                let mut node = HomeAssistant::create(name, NodeId(uuid::Uuid::nil())).unwrap();
                check(node.as_ref());
                // Also exercise every selected label, both signed sun offsets and all weekday chips.
                let selects = node
                    .definition()
                    .properties
                    .into_iter()
                    .filter_map(|prop| {
                        if let PropertyKind::Select(options) = prop.kind {
                            Some((prop.name, options))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                for (name, options) in selects {
                    for (value, _) in options {
                        node.set_property(&name, value.into());
                        check(node.as_ref());
                    }
                }
            }
        });
    }
    i18n::set_runtime_translations(&[]);
    i18n::set_known_types(&TypeRegistry::new());
}

#[test]
fn header_and_select_caps_keep_titles_and_property_labels_visible() {
    use ticknode_shared::PropertyDefinition;
    let mut def = NodeDefinition {
        type_name: "width/probe".into(),
        title: "Template trigger".into(),
        category: "Home Assistant".into(),
        description: String::new(),
        help: String::new(),
        pins: vec![],
        adaptive: false,
        cost: 1,
        properties: vec![PropertyDefinition::select(
            "value_type",
            "Value type",
            &[
                ("text", "Text"),
                ("long", "A very long unselected quantity label"),
            ],
        )],
    };
    let props = [("value_type".into(), "text".into())];
    let normal = NodeLayout::new(&def, &props);
    assert!(normal.width < 480.0);
    def.properties[0] = PropertyDefinition::select("value_type", "Value type", &[("text", "Text")]);
    assert_eq!(NodeLayout::new(&def, &props).width, normal.width);
    def.title = "Long template trigger title".into();
    assert!(NodeLayout::new(&def, &props).width > normal.width);
    def.category = "An extremely long plugin category badge".into();
    assert_eq!(NodeLayout::new(&def, &props).width, 480.0);
}
