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

    // Check resources
    for r in HomeAssistant::manifest().resources {
        let r_slug = fluent_slug(&r.id);
        let key = format!("home-assistant-resource-{r_slug}-title");
        if !ftl.contains(&format!("{key} =")) {
            missing.push(key);
        }
        for a in r.actions {
            let t_slug = fluent_slug(&a.type_name);
            let key = format!("home-assistant-resource-{r_slug}-action-{t_slug}");
            if !ftl.contains(&format!("{key} =")) {
                missing.push(key);
            }
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
        let mut keys: Vec<_> = ["title", "description", "help"]
            .map(|suffix| format!("{prefix}-{suffix}"))
            .into();
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
                if !ftl.contains(&format!("{key_raw} =")) && !ftl.contains(&format!("{key_slug} ="))
                {
                    keys.push(key_raw);
                }
            }
            if let PropertyKind::Select(options) = &property.kind {
                for (value, _) in options {
                    let opt_slug = fluent_slug(value);
                    let key_raw = format!("{prefix}-prop-{p_name}-option-{opt_slug}");
                    let key_slug =
                        format!("{prefix}-prop-{}-option-{opt_slug}", fluent_slug(p_name));
                    if !ftl.contains(&format!("{key_raw} ="))
                        && !ftl.contains(&format!("{key_slug} ="))
                    {
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

#[test]
fn invalid_action_names_return_the_visible_error_and_failed_signal() {
    let nil = NodeId(ticknode_shared::Uuid::nil());
    let mut node = HomeAssistant::create("ha/action", nil).unwrap();
    for name in ["", "light", ".turn_on", "light."] {
        node.set_property("action", name.into());
        assert_eq!(
            node.execute(Trigger::Pin(PinId(0)), &Inputs::default()),
            vec![
                Output::Data(
                    actions::ERROR,
                    "Choose a Home Assistant action (domain.service)".into()
                ),
                Output::Exec(actions::FAILED),
            ]
        );
    }
}

#[test]
fn connection_validation_returns_translatable_errors() {
    let error = validate_connection(DEFAULT_URL, "").unwrap_err();
    assert_eq!(error.key, "home-assistant-error-no-token");
    assert_eq!(error.fallback, "The access token is not set");
    let error = validate_connection("https://ha.local", "secret").unwrap_err();
    assert_eq!(error.key, "home-assistant-error-bad-url");
    assert_eq!(error.args, vec![("url".into(), "https://ha.local".into())]);
    assert!(validate_connection("ws://ha.local", "secret").is_ok());
    assert!(validate_connection("wss://ha.local", "secret").is_ok());
}

#[test]
fn a_valid_action_fails_promptly_while_unconfigured() {
    let nil = NodeId(ticknode_shared::Uuid::nil());
    let mut node = HomeAssistant::create("ha/action", nil).unwrap();
    node.set_property("action", "light.turn_on".into());
    for inputs in [
        Inputs::default(),
        [(PinId(1), "light.hall".into())].into_iter().collect(),
    ] {
        let outputs = node.execute(Trigger::Pin(PinId(0)), &inputs);
        assert_eq!(outputs,vec![Output::Data(actions::ERROR,"Home Assistant is not configured (set the URL and the access token in the plugin settings)".into()),Output::Exec(actions::FAILED)]);
    }
}

#[test]
fn ha_manifest_exposes_collections_and_actions() {
    let manifest = HomeAssistant::manifest();
    assert_eq!(manifest.resources.len(), 3);

    let entities = manifest
        .resources
        .iter()
        .find(|r| r.id == "entities")
        .expect("entities collection");
    assert_eq!(entities.title, "Entities");
    assert_eq!(entities.icon, "mdi:home-assistant");
    assert_eq!(entities.source, entities::ENTITIES_SOURCE);
    assert_eq!(entities.actions.len(), 2);
    assert_eq!(entities.actions[0].type_name, "ha/entity");
    assert_eq!(entities.actions[0].property, "entity_id");
    assert_eq!(entities.actions[0].requires_source, None);
    assert_eq!(entities.actions[1].type_name, "ha/action");
    assert_eq!(entities.actions[1].property, "target");
    assert_eq!(
        entities.actions[1].requires_source.as_deref(),
        Some(entities::ACTION_TARGETS_SOURCE)
    );

    let zones = manifest
        .resources
        .iter()
        .find(|r| r.id == "zones")
        .expect("zones collection");
    assert_eq!(zones.title, "Zones");
    assert_eq!(zones.icon, "mdi:map-marker-radius");
    assert_eq!(zones.source, "zones");
    assert_eq!(zones.actions.len(), 2);

    let persons = manifest
        .resources
        .iter()
        .find(|r| r.id == "persons")
        .expect("persons collection");
    assert_eq!(persons.title, "People");
    assert_eq!(persons.icon, "mdi:account");
    assert_eq!(persons.source, "persons");
    assert_eq!(persons.actions.len(), 1);
    assert_eq!(persons.actions[0].type_name, "ha/condition_zone");
    assert_eq!(persons.actions[0].property, "entity_id");
}
