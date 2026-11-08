use super::*;
use ticknode_shared::fluent_slug;

#[test]
fn action_entity_and_condition_static_texts_have_ukrainian_translations() {
    let ftl = HomeAssistant::TRANSLATIONS[0].1;
    let mut missing = Vec::new();

    // Check category
    let cat_key = format!("category-{}", fluent_slug(CATEGORY));
    if !ftl.contains(&format!("{cat_key} =")) {
        missing.push(cat_key);
    }

    // Check settings
    for s in HomeAssistant::manifest().settings {
        let key = format!("setting-home-assistant-{}-title", fluent_slug(&s.key));
        if !ftl.contains(&format!("{key} =")) {
            missing.push(key);
        }
        let key = format!("setting-home-assistant-{}-description", fluent_slug(&s.key));
        if !ftl.contains(&format!("{key} =")) {
            missing.push(key);
        }
    }

    // Check types
    for ty in HomeAssistant::types() {
        let ty_slug = fluent_slug(&ty.name);
        let key = format!("type-{ty_slug}-title");
        if !ftl.contains(&format!("{key} =")) {
            missing.push(key);
        }
        let key = format!("type-{ty_slug}-description");
        if !ftl.contains(&format!("{key} =")) {
            missing.push(key);
        }
        for f in ty.fields() {
            let key = format!("type-{ty_slug}-field-{}", f.name);
            if !ftl.contains(&format!("{key} =")) {
                missing.push(key);
            }
        }
    }

    for kind in HomeAssistant::NODE_TYPES {
        let node = HomeAssistant::create(kind, NodeId(ticknode_shared::Uuid::nil())).unwrap();
        let def = node.definition();
        let prefix = format!("node-{}", fluent_slug(kind));
        let mut keys: Vec<_> = ["title", "description", "help"].map(|suffix| format!("{prefix}-{suffix}")).into();
        for p in &def.pins {
            let key_raw = format!("{prefix}-pin-{}", p.name);
            let key_slug = format!("{prefix}-pin-{}", fluent_slug(&p.name));
            if !ftl.contains(&format!("{key_raw} =")) && !ftl.contains(&format!("{key_slug} =")) {
                keys.push(key_raw);
            }
        }
        for property in def.properties {
            let p_name = &property.name;
            for suffix in ["title", "description"] {
                let key_raw = format!("{prefix}-prop-{p_name}-{suffix}");
                let key_slug = format!("{prefix}-prop-{}-{suffix}", fluent_slug(p_name));
                if !ftl.contains(&format!("{key_raw} =")) && !ftl.contains(&format!("{key_slug} =")) {
                    keys.push(key_raw);
                }
            }
            if let PropertyKind::Select(options) = &property.kind {
                for (value, _) in options {
                    let opt_slug = fluent_slug(value);
                    let key_raw = format!("{prefix}-prop-{p_name}-option-{opt_slug}");
                    let key_slug = format!("{prefix}-prop-{}-option-{opt_slug}", fluent_slug(p_name));
                    if !ftl.contains(&format!("{key_raw} =")) && !ftl.contains(&format!("{key_slug} =")) {
                        keys.push(key_raw);
                    }
                }
            }
        }
        for key in keys {
            if !ftl.contains(&format!("{key} =")) {
                missing.push(key);
            }
        }
    }
    assert!(missing.is_empty(), "missing keys: {:#?}", missing);
}
