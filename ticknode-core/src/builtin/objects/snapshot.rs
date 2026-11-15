use serde::{Deserialize, Serialize};
use ticknode_shared::FieldDefinition;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldsSnapshot {
    pub title: String,
    pub fields: Vec<FieldDefinition>,
}

pub fn encode_fields_snapshot(title: &str, fields: &[FieldDefinition]) -> Option<String> {
    if fields.is_empty() {
        return None;
    }
    let snapshot = FieldsSnapshot {
        title: title.to_string(),
        fields: fields.to_vec(),
    };
    serde_json::to_string(&snapshot).ok()
}

pub fn decode_fields_snapshot(json: &str) -> Option<FieldsSnapshot> {
    serde_json::from_str::<FieldsSnapshot>(json).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ticknode_shared::DataType;

    #[test]
    fn encode_decode_roundtrip() {
        let fields = vec![
            FieldDefinition::new("x", "X", DataType::Number),
            FieldDefinition::new("label", "Label", DataType::String).optional(),
        ];
        let json = encode_fields_snapshot("Point", &fields).expect("json string");
        let decoded = decode_fields_snapshot(&json).expect("decoded snapshot");
        assert_eq!(decoded.title, "Point");
        assert_eq!(decoded.fields, fields);
    }

    #[test]
    fn empty_fields_return_none() {
        assert_eq!(encode_fields_snapshot("Point", &[]), None);
    }

    #[test]
    fn corrupt_json_ignored() {
        assert_eq!(decode_fields_snapshot("not a json"), None);
        assert_eq!(decode_fields_snapshot(r#"{"title": "incomplete""#), None);
        assert_eq!(
            decode_fields_snapshot(r#"{"title": 123, "fields": []}"#),
            None
        );
    }
}
