use thiserror::Error;
use ticknode_shared::*;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum EngineError {
    #[error("Circular reference: node {0:?} already depends on {1:?}")]
    CycleDetected(NodeId, NodeId),
    #[error("Incompatible data types: cannot connect {0:?} to {1:?}")]
    TypeMismatch(DataType, DataType),
    #[error(
        "Invalid pin connection: one or both pins do not exist or have the wrong direction/type"
    )]
    InvalidPinConnection,
    #[error("Node {0:?} not found")]
    NodeNotFound(NodeId),
    #[error("Unknown node type: {0}")]
    UnknownNodeType(String),
    #[error("Node unavailable: {0}; it cannot be changed")]
    PluginUnavailable(UiMessage),
    #[error("{0}")]
    InvalidProperty(UiMessage),
}

impl EngineError {
    /// The error for the user in the editor: a key from the editor's `ui.ftl` (`error-...`) with arguments
    /// and the English text (same as `Display`) as the fallback.
    pub fn ui_message(&self) -> UiMessage {
        let text = self.to_string();
        match self {
            Self::CycleDetected(a, b) => UiMessage::new("error-cycle-detected", text)
                .arg("node", format!("{a:?}"))
                .arg("dependency", format!("{b:?}")),
            Self::TypeMismatch(a, b) => UiMessage::new("error-type-mismatch", text)
                .arg("src", data_type_name(a))
                .arg("dst", data_type_name(b)),
            Self::InvalidPinConnection => UiMessage::new("error-invalid-pin-connection", text),
            Self::NodeNotFound(id) => {
                UiMessage::new("error-node-not-found", text).arg("node", format!("{id:?}"))
            }
            Self::UnknownNodeType(name) => {
                UiMessage::new("error-unknown-node-type", text).arg("type", name)
            }
            // The stub's own message already says which plugin is missing
            Self::PluginUnavailable(reason) | Self::InvalidProperty(reason) => reason.clone(),
        }
    }
}
