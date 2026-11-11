//! Pure update filtering and typed projection. Missing optional outputs explicitly clear caches.
use crate::{
    model::{Message, Update},
    validation::{self, Properties},
};
use ticknode_plugin_sdk::*;

pub struct Filter {
    kind: String,
    props: Properties,
    regex: Option<regex::Regex>,
}
impl Filter {
    pub fn new(kind: &str, props: Properties) -> Result<Self, UiError> {
        validation::stored(kind, &props)?;
        let regex = if props.get("text_mode").is_some_and(|m| m == "regex") {
            Some(
                regex::RegexBuilder::new(props.get("pattern").map(String::as_str).unwrap_or(""))
                    .size_limit(256_000)
                    .build()
                    .map_err(|_| validation::error("regex", "Invalid regular expression"))?,
            )
        } else {
            None
        };
        Ok(Self {
            kind: kind.into(),
            props,
            regex,
        })
    }
    fn prop(&self, name: &str) -> &str {
        self.props.get(name).map(String::as_str).unwrap_or("")
    }
    pub fn toast(&self) -> &str {
        self.prop("toast")
    }
    fn id_matches(&self, name: &str, id: Option<i64>) -> bool {
        self.prop(name).is_empty()
            || self
                .prop(name)
                .parse::<i64>()
                .ok()
                .is_some_and(|wanted| Some(wanted) == id)
    }
    pub fn outputs(&self, update: &Update, bot: &str) -> Option<Vec<Output>> {
        let callback = update.callback_query.as_ref();
        let message = update.message();
        let from = if self.kind == "callback" {
            callback.map(|c| &c.from)
        } else {
            message.and_then(|m| m.from.as_ref())
        };
        if !self.id_matches("chat", message.map(|m| m.chat.id))
            || !self.id_matches("topic", message.and_then(|m| m.message_thread_id))
            || !self.id_matches("user", from.map(|u| u.id))
        {
            return None;
        }
        let mut outputs = match self.kind.as_str() {
            "message" => {
                if callback.is_some() || update.my_chat_member.is_some() {
                    return None;
                }
                if update.edited_message.is_some() && self.prop("include_edited") != "yes" {
                    return None;
                }
                let text = message?.text.as_deref().unwrap_or("");
                let pattern = self.prop("pattern");
                let matches = match self.prop("text_mode") {
                    "any" => true,
                    "equals" => text == pattern,
                    "contains" => text.contains(pattern),
                    "starts_with" => text.starts_with(pattern),
                    "regex" => self.regex.as_ref()?.is_match(text),
                    _ => false,
                };
                if !matches {
                    return None;
                }
                vec![opt(1, message?.text.clone().map(DataValue::from))]
            }
            "command" => {
                if update.message.is_none() {
                    return None;
                }
                let arguments = command_args(message?, self.prop("command"), bot)?;
                vec![
                    Output::Data(PinId(1), arguments.into()),
                    Output::Data(
                        PinId(2),
                        DataValue::List {
                            element_type: DataType::String,
                            elements: validation::args(arguments)
                                .into_iter()
                                .map(DataValue::from)
                                .collect(),
                        },
                    ),
                ]
            }
            "callback" => {
                let data = callback?.data.as_deref()?;
                let matches = if self.prop("data_mode") == "prefix" {
                    data.starts_with(self.prop("pattern"))
                } else {
                    data == self.prop("pattern")
                };
                if !matches {
                    return None;
                }
                vec![Output::Data(PinId(1), data.into())]
            }
            _ => return None,
        };
        outputs.extend([
            opt(3, message.map(Message::value)),
            opt(4, message.map(|m| m.chat.id.into())),
            opt(
                5,
                message
                    .and_then(|m| m.message_thread_id)
                    .map(DataValue::from),
            ),
            opt(6, from.map(|u| u.value())),
            Output::Exec(PinId(0)),
        ]);
        Some(outputs)
    }
}
fn opt(pin: u32, value: Option<DataValue>) -> Output {
    value
        .map(|v| Output::Data(PinId(pin), v))
        .unwrap_or(Output::Clear(PinId(pin)))
}
pub fn command_args<'a>(message: &'a Message, expected: &str, bot: &str) -> Option<&'a str> {
    let text = message.text.as_deref()?;
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    let command = text[..end].strip_prefix('/')?;
    let (name, mention) = command
        .split_once('@')
        .map(|(a, b)| (a, Some(b)))
        .unwrap_or((command, None));
    if name != validation::command(expected)?
        || mention.is_some_and(|m| !m.eq_ignore_ascii_case(bot))
    {
        return None;
    }
    Some(text[end..].trim_start())
}
