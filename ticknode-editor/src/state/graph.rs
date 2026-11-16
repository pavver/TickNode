//! Graph selection, node lookup and pin connectivity queries.

use super::*;

/// Pure check whether a required data input pin lacks both a connection and an inline value or variable binding.
pub fn is_required_missing(
    pin: &PinDefinition,
    is_connected: bool,
    properties: &[(String, DataValue)],
) -> bool {
    if !pin.required || pin.direction != PinDirection::Input || pin.pin_type == PinType::Exec {
        return false;
    }
    let val_prop = input_value_property(pin.id);
    let var_prop = input_variable_property(pin.id);
    !is_connected
        && !properties
            .iter()
            .any(|(p, _)| *p == val_prop || *p == var_prop)
}

impl Editor {
    /// Whether the node is selected (tracked).
    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selection
            .with(|s| matches!(s, Some(Selection::Nodes(set)) if set.contains(&id)))
    }

    pub fn selected_nodes(&self) -> Vec<NodeId> {
        self.selection.with_untracked(|s| match s {
            Some(Selection::Nodes(set)) => set.iter().copied().collect(),
            _ => Vec::new(),
        })
    }

    pub fn select_nodes(&self, ids: impl IntoIterator<Item = NodeId>) {
        let set: BTreeSet<NodeId> = ids.into_iter().collect();
        self.selection
            .set((!set.is_empty()).then_some(Selection::Nodes(set)));
    }

    /// Removes deleted nodes and wires referencing them from the current selection.
    pub(super) fn prune_selection(&self) {
        let exists = |id: &NodeId| self.node(*id).is_some();
        let next = match self.selection.get_untracked() {
            Some(Selection::Nodes(set)) => {
                let kept: BTreeSet<_> = set.iter().copied().filter(exists).collect();
                if kept.len() == set.len() {
                    return;
                }
                (!kept.is_empty()).then_some(Selection::Nodes(kept))
            }
            Some(Selection::Exec(c)) if !self.exec.with_untracked(|e| e.contains(&c)) => None,
            Some(Selection::Data(c)) if !self.data.with_untracked(|d| d.contains(&c)) => None,
            Some(Selection::Detached(d))
                if !self.detached.with_untracked(|all| all.contains(&d)) =>
            {
                None
            }
            Some(Selection::Frame(id)) if !self.frames.with_untracked(|f| f.contains_key(&id)) => {
                None
            }
            _ => return,
        };
        self.selection.set(next);
    }

    /// Node lookup without subscribing to the node map (event handlers, applying messages).
    pub fn node(&self, id: NodeId) -> Option<ClientNode> {
        self.nodes.with_untracked(|nodes| nodes.get(&id).copied())
    }

    /// Tracked node lookup: for computations that should update when the node appears or disappears.
    pub fn node_tracked(&self, id: NodeId) -> Option<ClientNode> {
        self.nodes.with(|nodes| nodes.get(&id).copied())
    }

    pub fn node_title(&self, id: NodeId) -> String {
        self.node(id)
            .map(|n| {
                n.instance
                    .with_untracked(|i| i18n::node_title(&i.definition))
            })
            .unwrap_or_else(|| "?".to_string())
    }

    pub fn is_input_connected(&self, node: NodeId, pin: PinId) -> bool {
        self.data
            .with(|d| d.iter().any(|c| c.dst_node == node && c.dst_pin == pin))
            || self
                .exec
                .with(|e| e.iter().any(|c| c.dst_node == node && c.dst_pin == pin))
    }

    /// Data type of the output pin connected to a data input (tracked).
    pub fn source_type(&self, node: NodeId, pin: PinId) -> Option<DataType> {
        let src = self.data.with(|d| {
            d.iter()
                .find(|c| c.dst_node == node && c.dst_pin == pin)
                .map(|c| (c.src_node, c.src_pin))
        });
        let Some(src) = src else {
            return self.bound_input_type(node, pin);
        };
        self.output_type(src.0, src.1)
    }

    /// Required data input that is neither connected nor has an inline value or variable binding on the node (tracked).
    pub fn required_missing(&self, node: NodeId, pin: &PinDefinition) -> bool {
        if !pin.required || pin.direction != PinDirection::Input || pin.pin_type == PinType::Exec {
            return false;
        }
        let is_connected = self.is_input_connected(node, pin.id);
        if is_connected {
            return false;
        }
        !self.node_tracked(node).is_some_and(|n| {
            n.instance
                .with(|i| !is_required_missing(pin, is_connected, &i.model.properties))
        })
    }

    /// Human-readable type name (`title` for custom types), tracked against the type registry.
    pub fn type_title(&self, t: &DataType) -> String {
        self.types.with(|types| i18n::data_type_title(types, t))
    }

    pub fn is_output_connected(&self, node: NodeId, pin: PinId) -> bool {
        self.data
            .with(|d| d.iter().any(|c| c.src_node == node && c.src_pin == pin))
            || self
                .exec
                .with(|e| e.iter().any(|c| c.src_node == node && c.src_pin == pin))
    }
}

impl Editor {
    /// All canvas consumers use the same connection-aware layout, including live resize previews.
    pub fn layout(&self, id: NodeId) -> Option<crate::geometry::NodeLayout> {
        self.node_tracked(id).map(|node| self.layout_of(node))
    }

    pub fn layout_of(&self, node: ClientNode) -> crate::geometry::NodeLayout {
        node.instance.with(|i| {
            let connected = self.data.with(|all| {
                all.iter()
                    .filter(|c| c.dst_node == i.model.id)
                    .map(|c| c.dst_pin)
                    .collect::<Vec<_>>()
            });
            crate::geometry::NodeLayout::sized(
                &i.definition,
                &i.model.properties,
                node.size_preview.get().or(i.model.size),
                &connected,
            )
        })
    }
}
