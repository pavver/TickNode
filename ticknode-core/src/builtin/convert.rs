use crate::{Node, NodeContext};
use ticknode_shared::{
    convert::{self, ConversionTarget},
    *,
};

pub struct ConvertNode {
    id: NodeId,
    value_type: DataType,
    target: String,
    unit: String,
    restoring: bool,
}

impl ConvertNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            value_type: DataType::Any,
            target: String::new(),
            unit: String::new(),
            restoring: true,
        }
    }

    fn targets(&self) -> Vec<ConversionTarget> {
        convert::targets(&self.value_type, &TypeRegistry::default())
    }

    fn reset_unit(&mut self) {
        self.unit = self
            .targets()
            .iter()
            .find(|t| t.id == self.target)
            .and_then(|t| t.unit_of.as_deref())
            .and_then(quantity::kind)
            .map(|q| q.canonical.clone())
            .unwrap_or_default();
    }
}

impl Node for ConvertNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn is_pure(&self) -> bool {
        true
    }

    fn definition(&self) -> NodeDefinition {
        let targets = self.targets();
        let selected = targets.iter().find(|t| t.id == self.target);
        let options: Vec<_> = targets
            .iter()
            .map(|t| (t.id.as_str(), t.title.as_str()))
            .collect();
        let units: Vec<_> = selected
            .and_then(|t| t.unit_of.as_deref())
            .and_then(quantity::kind)
            .map(|q| {
                q.units
                    .iter()
                    .map(|u| (u.id.as_str(), u.symbol.as_str()))
                    .collect()
            })
            .unwrap_or_default();
        let properties = vec![
            PropertyDefinition::text("value_type", "Value type")
                .described("Declared input type; connecting a wire adapts it."),
            PropertyDefinition::select("target", "To", &options)
                .described("Only conversions available for the input type are listed."),
            PropertyDefinition::select("unit", "Unit", &units)
                .described("Unit used to interpret or express the number."),
        ]
        .into_iter()
        .filter(|p| convert::property_visible(&self.value_type, &self.target, &p.name))
        .collect();
        NodeDefinition {
            type_name: "convert".into(), title: "Convert".into(), category: "Data".into(),
            description: "Converts values using explicit, directional conversions".into(),
            help: "Connect Value to select its type, then choose To. Only available conversions are listed. Quantity → Number expresses the value in Unit; Number → Quantity interprets the number in Unit. Some conversions are one-way: color temperature → RGB exists, RGB → color temperature does not. Speed ↔ Beaufort and signal strength ↔ power convert between quantities. Date and time → Time uses the server local time zone; Unix time and durations use seconds. Tick durations have no seconds conversion. Missing, invalid or non-finite values produce no result.".into(),
            adaptive: true, cost: 1, properties,
            pins: vec![
                PinDefinition { id: PinId(1), name: "Value".into(), pin_type: PinType::Data(self.value_type.clone()), direction: PinDirection::Input, required: true },
                PinDefinition { id: PinId(0), name: "Result".into(), pin_type: PinType::Data(selected.map(|t| t.output.clone()).unwrap_or(DataType::Any)), direction: PinDirection::Output, required: false },
            ],
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        ctx.get_value(PinId(1))
            .and_then(|v| {
                convert::convert(
                    &v,
                    &self.value_type,
                    &self.target,
                    Some(&self.unit),
                    ctx.types,
                )
            })
            .map(|v| vec![NodeOutput::Data(PinId(0), v)])
            .unwrap_or_default()
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        [
            ("value_type", data_type_name(&self.value_type)),
            ("target", self.target.clone()),
            ("unit", self.unit.clone()),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect()
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        let DataValue::String(s) = value else { return };
        match name {
            "value_type" => {
                if let Some(t) = parse_data_type(&s) {
                    self.value_type = t;
                }
            }
            "target" => {
                let changed = self.target != s;
                self.target = s;
                // Live target selection starts in canonical units; restoration preserves stored units
                // regardless of the order in which graph properties are applied.
                if changed && !self.restoring {
                    self.reset_unit();
                }
            }
            "unit" => self.unit = s,
            _ => {}
        }
    }

    fn restored(&mut self) {
        self.restoring = false;
    }
    fn adapts_on_connect(&self, pin: PinId) -> bool {
        pin == PinId(1)
    }

    fn adapt_to(&mut self, pin: PinId, t: &DataType) -> bool {
        if pin != PinId(1) || *t == self.value_type {
            return false;
        }
        self.value_type = t.clone();
        let targets = self.targets();
        if !targets.iter().any(|t| t.id == self.target) {
            self.target = targets.first().map(|t| t.id.clone()).unwrap_or_default();
            self.reset_unit();
        } else if targets
            .iter()
            .find(|t| t.id == self.target)
            .and_then(|t| t.unit_of.as_deref())
            .and_then(quantity::kind)
            .is_some_and(|q| !q.units.iter().any(|u| u.id == self.unit))
        {
            self.reset_unit();
        }
        true
    }
}
