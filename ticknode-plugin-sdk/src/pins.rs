use crate::{PinType, PinDirection, PinDefinition, PinId, DataType};

/// Shorthand helpers for defining pins in `definition()`.
pub fn pin(id: u32, name: &str, pin_type: PinType, direction: PinDirection) -> PinDefinition {
    PinDefinition { id: PinId(id), name: name.to_string(), pin_type, direction, required: false }
}

pub fn exec_in(id: u32, name: &str) -> PinDefinition {
    pin(id, name, PinType::Exec, PinDirection::Input)
}

pub fn exec_out(id: u32, name: &str) -> PinDefinition {
    pin(id, name, PinType::Exec, PinDirection::Output)
}

pub fn data_in(id: u32, name: &str, t: DataType) -> PinDefinition {
    pin(id, name, PinType::Data(t), PinDirection::Input)
}

pub fn data_out(id: u32, name: &str, t: DataType) -> PinDefinition {
    pin(id, name, PinType::Data(t), PinDirection::Output)
}
