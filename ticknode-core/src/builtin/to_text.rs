use crate::{Node, NodeContext};
use ticknode_shared::{
    to_text::{ToTextOptions, property_visible, to_text_with_identifiers},
    *,
};

pub struct ToTextNode {
    id: NodeId,
    value_type: DataType,
    options: ToTextOptions,
    locale: String,
}
impl ToTextNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            value_type: DataType::Any,
            options: Default::default(),
            locale: "server".into(),
        }
    }
}
impl Node for ToTextNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn is_pure(&self) -> bool {
        true
    }
    fn definition(&self) -> NodeDefinition {
        let mut units = vec![("auto", "Project rule")];
        let mut element = &self.value_type;
        while let DataType::List(inner) = element {
            element = inner;
        }
        if let DataType::Named(name) = element
            && let Some(q) = quantity::kind(name)
        {
            units.extend(q.units.iter().map(|u| (u.id.as_str(), u.symbol.as_str())));
        }
        let properties = vec![
            PropertyDefinition::text("value_type", "Value type")
                .described("Declared input type; connecting a wire adapts it."),
            PropertyDefinition::select("show_units", "Show units", &[("yes", "Yes"), ("no", "No")])
                .described("Append unit symbols; without units, durations use seconds."),
            PropertyDefinition::select("unit", "Unit", &units)
                .described("Empty or Auto uses the project display rule."),
            PropertyDefinition::select("rounding", "Rounding", &[
                ("project", "Project rule"),
                ("significant", "Significant digits"),
                ("decimals", "Decimal places"),
            ]).described("Use the project rule or choose a rounding mode."),
            PropertyDefinition::text("digits", "Digits")
                .described("Significant digits (1–15) or decimal places (0–15); empty uses the default for the selected rounding."),
            PropertyDefinition::select(
                "locale",
                "Language",
                &[("server", "Server"), ("en", "English"), ("uk", "Ukrainian")],
            )
            .described("Server follows the engine formatting language."),
            PropertyDefinition::text("true_text", "True text")
                .described("Empty uses the localized default."),
            PropertyDefinition::text("false_text", "False text")
                .described("Empty uses the localized default."),
            PropertyDefinition::text("datetime_format", "Date and time format")
                .described("Chrono strftime; empty uses %Y-%m-%d %H:%M or %H:%M."),
        ]
        .into_iter()
        .filter(|p| property_visible(&self.value_type, &p.name))
        .collect();
        NodeDefinition {
            type_name: "text/to_text".into(), title: "To text".into(), category: "String".into(),
            description: "Converts any value to readable text".into(),
            help: "Converts Value to text. Connect a wire to select its type. Quantities follow project display rules: 1970 W → 1.97 kW; a fixed unit overrides the rule. Numbers and quantities support significant digits or fixed decimal places. Project rounding with empty digits follows the project rule; a selected mode defaults to 3 significant digits or 2 decimal places. Hide units to output a bare number; durations then use seconds. Booleans use localized words unless custom texts are set. Date/time formats use chrono strftime, e.g. %H:%M or %d.%m.%Y; date-times use the server local time zone. Lists join elements with commas; objects show their fields. Server uses the engine formatting language.".into(),
            adaptive: true, properties,
            pins: vec![
                PinDefinition { id: PinId(1), name: "Value".into(), pin_type: PinType::Data(self.value_type.clone()), direction: PinDirection::Input, required: true },
                PinDefinition { id: PinId(0), name: "Text".into(), pin_type: PinType::Data(DataType::String), direction: PinDirection::Output, required: false },
            ], ..Default::default()
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let Some(value) = ctx.get_value(PinId(1)) else {
            return vec![];
        };
        let locale = if self.locale == "server" {
            &ctx.host_state.locale
        } else {
            &self.locale
        };
        let text = to_text_with_identifiers(
            &value,
            ctx.input_types.get(&PinId(1)).filter(|_| self.value_type == DataType::Any).unwrap_or(&self.value_type),
            &self.options,
            &ctx.host_state.display,
            crate::quantity::labels(locale),
            ctx.types,
            &ctx.host_state.identifier_labels,
        );
        vec![NodeOutput::Data(PinId(0), text.into())]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        [
            ("value_type", data_type_name(&self.value_type)),
            (
                "unit",
                self.options.unit.clone().unwrap_or_else(|| "auto".into()),
            ),
            (
                "rounding",
                match self.options.rounding {
                    Some(Rounding::Significant) => "significant",
                    Some(Rounding::Decimals) => "decimals",
                    None if self.options.digits.is_some() => "significant",
                    None => "project",
                }
                .into(),
            ),
            (
                "digits",
                self.options
                    .digits
                    .map(|n| {
                        if self.options.rounding == Some(Rounding::Decimals) {
                            n
                        } else {
                            n.max(1)
                        }
                        .to_string()
                    })
                    .unwrap_or_default(),
            ),
            ("locale", self.locale.clone()),
            (
                "show_units",
                if self.options.show_units { "yes" } else { "no" }.into(),
            ),
            ("true_text", self.options.true_text.clone()),
            ("false_text", self.options.false_text.clone()),
            ("datetime_format", self.options.datetime_format.clone()),
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
            "unit" => self.options.unit = (s != "auto").then_some(s),
            "rounding" => {
                self.options.rounding = match s.as_str() {
                    "project" => None,
                    "significant" => Some(Rounding::Significant),
                    "decimals" => Some(Rounding::Decimals),
                    _ => return,
                };
            }
            "digits" if s.is_empty() => self.options.digits = None,
            "digits" => {
                // Preserve zero until the mode is known: graph properties can restore in any order.
                if let Ok(n @ 0..=15) = s.parse::<u8>() {
                    self.options.digits = Some(n);
                }
            }
            "locale" if ["server", "en", "uk"].contains(&s.as_str()) => self.locale = s,
            "show_units" if ["yes", "no"].contains(&s.as_str()) => {
                self.options.show_units = s == "yes"
            }
            "true_text" => self.options.true_text = s,
            "false_text" => self.options.false_text = s,
            "datetime_format" => self.options.datetime_format = s,
            _ => {}
        }
    }
    fn adapts_on_connect(&self, pin: PinId) -> bool {
        pin == PinId(1)
    }
    fn adapt_to(&mut self, pin: PinId, t: &DataType) -> bool {
        if pin != PinId(1) || *t == self.value_type {
            return false;
        }
        self.value_type = t.clone();
        true
    }
}
