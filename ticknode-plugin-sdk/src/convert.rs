use crate::bindings::ticknode::plugin::types as wit;
use crate::*;

pub(crate) fn encode(value: &DataValue) -> Vec<u8> {
    postcard::to_allocvec(value).expect("DataValue always serializes")
}

pub(crate) fn decode(bytes: &[u8]) -> Option<DataValue> {
    postcard::from_bytes(bytes).ok()
}

fn to_wit_data_type(mut t: &DataType) -> wit::DataType {
    let mut list_depth = 0u8;
    while let DataType::List(inner) = t {
        list_depth += 1;
        t = inner;
    }
    let base = match t {
        DataType::Bool => wit::ScalarType::Bool,
        DataType::Number => wit::ScalarType::Number,
        DataType::String => wit::ScalarType::String,
        DataType::Time => wit::ScalarType::Time,
        DataType::DateTime => wit::ScalarType::Datetime,
        DataType::Duration => wit::ScalarType::Duration,
        DataType::Color => wit::ScalarType::Color,
        DataType::Object => wit::ScalarType::Object,
        DataType::Any => wit::ScalarType::Any,
        DataType::Named(name) => wit::ScalarType::Named(name.clone()),
        DataType::List(_) => unreachable!("lists are unwrapped above"),
    };
    wit::DataType { base, list_depth }
}

pub(crate) fn to_wit_setting(s: SettingDefinition) -> wit::SettingDefinition {
    wit::SettingDefinition {
        key: s.key,
        title: s.title,
        description: s.description,
        kind: match s.kind {
            SettingKind::Text => wit::SettingKind::Text,
            SettingKind::Number => wit::SettingKind::Number,
            SettingKind::Bool => wit::SettingKind::Bool,
            SettingKind::Secret => wit::SettingKind::Secret,
            SettingKind::Url => wit::SettingKind::Url,
            SettingKind::Select(options) => wit::SettingKind::Select(options),
        },
        required: s.required,
        default: s.default.as_ref().map(encode),
    }
}

pub(crate) fn to_wit_type_definition(t: TypeDefinition) -> wit::TypeDefinition {
    wit::TypeDefinition {
        name: t.name,
        title: t.title,
        description: t.description,
        color: t.color,
        kind: match t.kind {
            TypeKind::Quantity(_) => panic!("Plugins cannot declare quantity types"),
            TypeKind::Struct { fields } => wit::TypeKind::Structure(
                fields
                    .into_iter()
                    .map(|f| wit::FieldDefinition {
                        name: f.name,
                        title: f.title,
                        description: f.description,
                        data_type: to_wit_data_type(&f.data_type),
                        optional: f.optional,
                    })
                    .collect(),
            ),
            TypeKind::Enum { values } => wit::TypeKind::Enumeration(
                values
                    .into_iter()
                    .map(|v| wit::EnumValue {
                        value: v.value,
                        title: v.title,
                    })
                    .collect(),
            ),
        },
    }
}

pub(crate) fn to_wit_definition(d: NodeDefinition, polls: bool) -> wit::NodeDefinition {
    wit::NodeDefinition {
        cost: d.cost,
        type_name: d.type_name,
        title: d.title,
        category: d.category,
        description: d.description,
        help: d.help,
        pins: d
            .pins
            .into_iter()
            .map(|p| wit::Pin {
                id: p.id.0,
                required: p.required,
                name: p.name,
                pin_type: match &p.pin_type {
                    PinType::Exec => wit::PinType::Exec,
                    PinType::Data(t) => wit::PinType::Data(to_wit_data_type(t)),
                },
                direction: match p.direction {
                    PinDirection::Input => wit::PinDirection::Input,
                    PinDirection::Output => wit::PinDirection::Output,
                },
            })
            .collect(),
        polls,
        properties: d
            .properties
            .into_iter()
            .map(|p| wit::PropertyDefinition {
                input: p.input.map(|pin| pin.0),
                name: p.name,
                title: p.title,
                description: p.description,
                kind: match p.kind {
                    PropertyKind::Duration { signed } => wit::PropertyKind::Duration(signed),
                    PropertyKind::Time => wit::PropertyKind::Time,
                    PropertyKind::MultiSelect(options) => wit::PropertyKind::MultiSelect(options),
                    PropertyKind::Multiline => wit::PropertyKind::Multiline,
                    PropertyKind::Hidden => wit::PropertyKind::Hidden,
                    PropertyKind::Text => wit::PropertyKind::Text,
                    PropertyKind::Select(options) => wit::PropertyKind::Select(options),
                    PropertyKind::Dynamic { source, .. } => wit::PropertyKind::Dynamic(source),
                },
            })
            .collect(),
    }
}

pub(crate) fn to_wit_output(output: Output) -> wit::Output {
    match output {
        Output::Exec(pin) => wit::Output::Exec(pin.0),
        Output::Data(pin, value) => wit::Output::Data((pin.0, encode(&value))),
        Output::Clear(pin) => wit::Output::Clear(pin.0),
    }
}

pub(crate) fn to_wit_resource_action(a: ResourceAction) -> wit::ResourceAction {
    wit::ResourceAction {
        title: a.title,
        type_name: a.type_name,
        property: a.property,
        requires_source: a.requires_source,
    }
}

pub(crate) fn to_wit_resource_collection(c: ResourceCollection) -> wit::ResourceCollection {
    wit::ResourceCollection {
        id: c.id,
        title: c.title,
        icon: c.icon,
        source: c.source,
        actions: c.actions.into_iter().map(to_wit_resource_action).collect(),
    }
}
