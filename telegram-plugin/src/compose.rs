//! Pure Telegram message composer and keyboard builders.
pub mod button;
pub mod keyboard;
pub mod message;

#[cfg(test)]
mod tests;

pub use button::ButtonNode;
pub use keyboard::KeyboardNode;
pub use message::ComposeNode;
use ticknode_plugin_sdk::PluginNode;

pub fn create(kind: &str) -> Box<dyn PluginNode> {
    match kind {
        "button" => Box::new(ButtonNode::new()),
        "keyboard" => Box::new(KeyboardNode::new()),
        "compose" => Box::new(ComposeNode::new()),
        _ => panic!("unknown compose node kind: {kind}"),
    }
}
