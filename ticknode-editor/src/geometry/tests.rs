use super::*;

mod layout;
mod routes;

pub(super) fn def(type_name: &str, pins: Vec<(u32, PinType, PinDirection)>) -> NodeDefinition {
    NodeDefinition {
        adaptive: false,
        cost: 1,
        properties: Vec::new(),
        type_name: type_name.into(),
        title: "T".into(),
        category: "Data".into(),
        description: String::new(),
        help: String::new(),
        pins: pins
            .into_iter()
            .map(|(id, pin_type, direction)| PinDefinition {
                id: PinId(id),
                name: "p".into(),
                pin_type,
                direction,
                required: false,
            })
            .collect(),
    }
}

#[test]
fn every_builtin_pin_color_is_stable() {
    for (ty, expected) in [
        (DataType::Bool, "var(--error)"),
        (DataType::Number, "var(--accent)"),
        (DataType::String, "var(--warning)"),
        (DataType::Time, "var(--info)"),
        (DataType::DateTime, "var(--info)"),
        (DataType::Duration, "var(--info)"),
        (DataType::Color, "var(--magenta)"),
        (DataType::List(Box::new(DataType::Any)), "var(--success)"),
        (DataType::Object, "var(--text-secondary)"),
        (DataType::Any, "var(--border-strong)"),
        (
            DataType::Named("test.value".into()),
            "var(--text-secondary)",
        ),
    ] {
        assert_eq!(data_type_color(&ty), expected);
    }
    assert_eq!(
        pin_color(&TypeRegistry::new(), &PinType::Exec),
        "var(--text-primary)"
    );
}

#[test]
fn zoom_keeps_cursor_point_fixed() {
    let view = Camera {
        x: 30.0,
        y: -20.0,
        zoom: 1.0,
    };
    let before = view.to_world(200.0, 150.0);
    let after = view.zoom_at(200.0, 150.0, 1.5).to_world(200.0, 150.0);
    assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
}

#[test]
fn named_type_colors() {
    use ticknode_shared::TypeDefinition;
    let mut types = TypeRegistry::new();
    types
        .insert_all(&[
            TypeDefinition::new_enum("p.colored", "C", &[("a", "A")]).colored("#e0a030"),
            TypeDefinition::new_enum("p.plain", "P", &[("a", "A")]),
            TypeDefinition::new_enum("p.evil", "E", &[("a", "A")])
                .colored("red;} body{display:none"),
        ])
        .unwrap();
    let color = |n: &str| type_color(&types, &DataType::Named(n.into()));
    assert_eq!(color("p.colored"), "#e0a030");
    assert!(color("p.plain").starts_with("hsl("));
    assert_eq!(color("p.plain"), color("p.plain"), "the color is stable");
    assert!(
        color("p.evil").starts_with("hsl("),
        "an invalid color does not reach the CSS"
    );
    assert_ne!(color("p.plain"), color("p.evil"));
    assert_eq!(type_color(&types, &DataType::Number), "var(--accent)");
}

#[test]
fn wire_conversion_matches_connection_rules() {
    let reg = TypeRegistry::new();
    let data = PinType::Data;
    assert_eq!(
        wire_conversion(&reg, &data(DataType::String), &data(DataType::Number)),
        Some(true)
    );
    assert_eq!(
        wire_conversion(&reg, &data(DataType::Number), &data(DataType::Number)),
        Some(false)
    );
    assert_eq!(
        wire_conversion(&reg, &data(DataType::Any), &data(DataType::Number)),
        Some(false)
    );
    assert_eq!(
        wire_conversion(&reg, &data(DataType::Object), &data(DataType::Number)),
        None
    );
    assert_eq!(
        wire_conversion(&reg, &PinType::Exec, &PinType::Exec),
        Some(false)
    );
    assert_eq!(
        wire_conversion(&reg, &PinType::Exec, &data(DataType::Number)),
        None
    );
}

#[test]
fn zoom_is_clamped() {
    assert_eq!(Camera::default().zoom_at(0.0, 0.0, 100.0).zoom, MAX_ZOOM);
    assert_eq!(Camera::default().zoom_at(0.0, 0.0, 0.001).zoom, MIN_ZOOM);
}
