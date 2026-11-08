use ticknode_shared::*;
use tracing::warn;
use super::wit;

pub(super) fn encode_value(value: &DataValue) -> wit::Value {
    postcard::to_allocvec(value).expect("DataValue always serializes")
}

pub(super) fn decode_value(bytes: &[u8]) -> Option<DataValue> {
    postcard::from_bytes(bytes).inspect_err(|e| warn!("The plugin returned an invalid value: {e}")).ok()
}

pub(super) fn from_wit_data_type(t: wit::DataType) -> DataType {
    let mut ty = match t.base {
        wit::ScalarType::Bool => DataType::Bool,
        wit::ScalarType::Number => DataType::Number,
        wit::ScalarType::String => DataType::String,
        wit::ScalarType::Time => DataType::Time,
        wit::ScalarType::Datetime => DataType::DateTime,
        wit::ScalarType::Duration => DataType::Duration,
        wit::ScalarType::Color => DataType::Color,
        wit::ScalarType::Object => DataType::Object,
        wit::ScalarType::Any => DataType::Any,
        wit::ScalarType::Named(name) => DataType::Named(name),
    };
    for _ in 0..t.list_depth {
        ty = DataType::List(Box::new(ty));
    }
    ty
}

pub(super) fn from_wit_type_definition(t: wit::TypeDefinition) -> TypeDefinition {
    TypeDefinition {
        name: t.name,
        title: t.title,
        description: t.description,
        color: t.color,
        kind: match t.kind {
            wit::TypeKind::Structure(fields) => TypeKind::Struct {
                fields: fields.into_iter().map(|f| FieldDefinition {
                    name: f.name,
                    title: f.title,
                    description: f.description,
                    data_type: from_wit_data_type(f.data_type),
                    optional: f.optional,
                }).collect(),
            },
            wit::TypeKind::Enumeration(values) => TypeKind::Enum {
                values: values.into_iter().map(|v| EnumValue { value: v.value, title: v.title }).collect(),
            },
        },
    }
}

/// Definition and `polls` flag (not part of `NodeDefinition`: it concerns the host, not the editor).
pub(super) fn from_wit_definition(d: wit::NodeDefinition, plugin: &str) -> (NodeDefinition, bool) {
    let definition = NodeDefinition {
        properties: d.properties.into_iter().map(|p| PropertyDefinition {
            name: p.name,
            title: p.title,
            description: p.description,
            kind: match p.kind {
                wit::PropertyKind::Text => PropertyKind::Text,
                wit::PropertyKind::Select(options) => PropertyKind::Select(options),
                wit::PropertyKind::Dynamic(source) => PropertyKind::Dynamic { source, plugin: plugin.to_string() },
            },
        }).collect(),
        type_name: d.type_name,
        title: d.title,
        category: d.category,
        description: d.description,
        help: d.help,
        pins: d.pins.into_iter().filter(|p| {
            if is_virtual_pin(PinId(p.id)) {
                tracing::warn!(plugin, pin = p.id, "Ignoring plugin pin in reserved virtual id range");
                false
            } else { true }
        }).map(|p| PinDefinition {
            id: PinId(p.id),
            required: p.required,
            name: p.name,
            pin_type: match p.pin_type {
                wit::PinType::Exec => PinType::Exec,
                wit::PinType::Data(t) => PinType::Data(from_wit_data_type(t)),
            },
            direction: match p.direction {
                wit::PinDirection::Input => PinDirection::Input,
                wit::PinDirection::Output => PinDirection::Output,
            } }).collect(),
    };
    (definition, d.polls)
}

pub(super) fn from_wit_manifest(m: wit::PluginManifest) -> PluginManifest {
    PluginManifest {
        name: m.name,
        version: m.version,
        author: m.author,
        requested_capabilities: m.capabilities.into_iter().map(|c| match c {
            wit::Capability::Http(allowed_domains) => Capability::Http { allowed_domains },
            wit::Capability::Storage => Capability::Storage,
            wit::Capability::Websocket(allowed_hosts) => Capability::WebSocket { allowed_hosts },
        }).collect(),
        settings: m.settings.into_iter().map(from_wit_setting).collect(),
    }
}

fn from_wit_setting(s: wit::SettingDefinition) -> SettingDefinition {
    SettingDefinition {
        key: s.key,
        title: s.title,
        description: s.description,
        kind: match s.kind {
            wit::SettingKind::Text => SettingKind::Text,
            wit::SettingKind::Number => SettingKind::Number,
            wit::SettingKind::Bool => SettingKind::Bool,
            wit::SettingKind::Secret => SettingKind::Secret,
            wit::SettingKind::Url => SettingKind::Url,
            wit::SettingKind::Select(options) => SettingKind::Select(options),
        },
        required: s.required,
        default: s.default.as_deref().and_then(decode_value),
    }
}
