//! Explicit, directional conversions shared by the engine and editor.

use crate::{DataType, DataValue, TypeDefinition, TypeRegistry, parse_data_type, quantity};

mod functions;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionTarget {
    pub id: String,
    pub title: String,
    pub output: DataType,
    pub unit_of: Option<String>,
}

enum Input {
    Type(DataType),
    Quantity(&'static str),
    AnyQuantity,
}

enum Target {
    NumberInUnit,
    Quantities,
    Fixed {
        id: &'static str,
        title: &'static str,
        output: &'static str,
        apply: fn(&DataValue) -> Option<DataValue>,
    },
}

struct Entry {
    input: Input,
    target: Target,
}

// Each fixed converter is one directional entry; no reverse conversion is inferred.
const ENTRIES: &[Entry] = &[
    Entry {
        input: Input::AnyQuantity,
        target: Target::NumberInUnit,
    },
    Entry {
        input: Input::Type(DataType::Number),
        target: Target::Quantities,
    },
    Entry {
        input: Input::Quantity("quantity.speed"),
        target: Target::Fixed {
            id: "beaufort",
            title: "Beaufort force",
            output: "quantity.beaufort",
            apply: functions::beaufort,
        },
    },
    Entry {
        input: Input::Quantity("quantity.beaufort"),
        target: Target::Fixed {
            id: "speed",
            title: "Speed",
            output: "quantity.speed",
            apply: functions::speed,
        },
    },
    Entry {
        input: Input::Quantity("quantity.signal_strength"),
        target: Target::Fixed {
            id: "power",
            title: "Power",
            output: "quantity.power",
            apply: functions::power,
        },
    },
    Entry {
        input: Input::Quantity("quantity.power"),
        target: Target::Fixed {
            id: "signal_strength",
            title: "Signal strength",
            output: "quantity.signal_strength",
            apply: functions::signal_strength,
        },
    },
    Entry {
        input: Input::Quantity("quantity.color_temperature"),
        target: Target::Fixed {
            id: "color",
            title: "Color (RGB)",
            output: "color",
            apply: functions::color,
        },
    },
    Entry {
        input: Input::Type(DataType::DateTime),
        target: Target::Fixed {
            id: "time",
            title: "Time of day",
            output: "time",
            apply: functions::time,
        },
    },
    Entry {
        input: Input::Type(DataType::DateTime),
        target: Target::Fixed {
            id: "unix_seconds",
            title: "Unix time (seconds)",
            output: "number",
            apply: functions::unix_seconds,
        },
    },
    Entry {
        input: Input::Type(DataType::Number),
        target: Target::Fixed {
            id: "datetime",
            title: "Date and time from Unix seconds",
            output: "datetime",
            apply: functions::datetime,
        },
    },
    Entry {
        input: Input::Type(DataType::Duration),
        target: Target::Fixed {
            id: "seconds",
            title: "Seconds",
            output: "number",
            apply: functions::seconds,
        },
    },
    Entry {
        input: Input::Type(DataType::Number),
        target: Target::Fixed {
            id: "duration",
            title: "Duration from seconds",
            output: "duration",
            apply: functions::duration,
        },
    },
];

fn quantity_definition<'a>(name: &str, types: &'a TypeRegistry) -> Option<&'a TypeDefinition> {
    types
        .get(name)
        .filter(|d| d.quantity().is_some())
        .or_else(|| quantity::catalog().iter().find(|d| d.name == name))
}

impl Input {
    fn matches(&self, input: &DataType, types: &TypeRegistry) -> bool {
        match (self, input) {
            (Self::Type(t), _) => input == t,
            (Self::Quantity(name), DataType::Named(n)) => n == name,
            (Self::AnyQuantity, DataType::Named(n)) => quantity_definition(n, types).is_some(),
            _ => false,
        }
    }
}

impl Entry {
    fn targets(&self, input: &DataType, types: &TypeRegistry) -> Vec<ConversionTarget> {
        if !self.input.matches(input, types) {
            return vec![];
        }
        match &self.target {
            Target::NumberInUnit => {
                let DataType::Named(name) = input else {
                    return vec![];
                };
                vec![ConversionTarget {
                    id: "number".into(),
                    title: "Number in unit".into(),
                    output: DataType::Number,
                    unit_of: Some(name.clone()),
                }]
            }
            Target::Quantities => quantity::catalog()
                .iter()
                .map(|d| ConversionTarget {
                    id: format!("quantity:{}", d.name),
                    title: d.title.clone(),
                    output: DataType::Named(d.name.clone()),
                    unit_of: Some(d.name.clone()),
                })
                .collect(),
            Target::Fixed {
                id, title, output, ..
            } => vec![ConversionTarget {
                id: (*id).into(),
                title: (*title).into(),
                output: parse_data_type(output).expect("catalog output type"),
                unit_of: None,
            }],
        }
    }
}

/// Available conversions in stable catalog order. `Any` waits for a declared input type.
pub fn targets(input: &DataType, types: &TypeRegistry) -> Vec<ConversionTarget> {
    ENTRIES
        .iter()
        .flat_map(|entry| entry.targets(input, types))
        .collect()
}

/// Converts a value using its declared type. Missing units, invalid values and non-finite results fail.
pub fn convert(
    value: &DataValue,
    input: &DataType,
    target: &str,
    unit: Option<&str>,
    types: &TypeRegistry,
) -> Option<DataValue> {
    for entry in ENTRIES {
        let Some(selected) = entry
            .targets(input, types)
            .into_iter()
            .find(|t| t.id == target)
        else {
            continue;
        };
        return match &entry.target {
            Target::NumberInUnit | Target::Quantities => {
                let name = selected.unit_of.as_deref()?;
                let q = quantity_definition(name, types)?.quantity()?;
                let unit = unit?;
                let u = q.units.iter().find(|u| u.id == unit)?;
                let n = functions::number(value)?;
                functions::finite(if matches!(entry.target, Target::NumberInUnit) {
                    quantity::from_canonical(n, u)
                } else {
                    quantity::to_canonical(n, u)
                })
            }
            Target::Fixed { apply, .. } => apply(value),
        };
    }
    None
}

/// Shared with editor geometry: persisted hidden properties must not reserve rows.
pub fn property_visible(input: &DataType, target: &str, name: &str) -> bool {
    match name {
        "value_type" => false,
        "unit" => targets(input, &TypeRegistry::default())
            .iter()
            .any(|t| t.id == target && t.unit_of.is_some()),
        _ => true,
    }
}
