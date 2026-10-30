//! Ноди-заглушки: нода, тип якої зараз недоступний (плагін вимкнено чи не завантажено).
//!
//! Заглушка тримає тип, усі властивості (включно зі значеннями входів `in:*`) і останню відому
//! дефініцію (піни), тож з'єднання лишаються, а граф записується на диск без змін. Вона не
//! виконується й не приймає змін; коли тип знову з'являється в реєстрі, замість неї
//! створюється справжня нода з тих самих властивостей.

use std::collections::HashSet;

use ticknode_shared::*;

use crate::{EngineError, Node, NodeContext, RemovedConnections, TickEngine};

pub struct StubNode {
    id: NodeId,
    definition: NodeDefinition,
    properties: Vec<(String, DataValue)>,
    /// Плагін, що вимкнено; `None` — тип просто невідомий (плагіна не завантажено).
    plugin: Option<String>,
}

impl StubNode {
    pub fn new(id: NodeId, definition: NodeDefinition, properties: Vec<(String, DataValue)>, plugin: Option<String>) -> Self {
        Self { id, definition, properties, plugin }
    }

    /// Дефініція для типу, про який відомі лише піни з з'єднань.
    pub fn bare_definition(type_name: &str, pins: Vec<PinDefinition>) -> NodeDefinition {
        NodeDefinition {
            type_name: type_name.into(),
            title: type_name.into(),
            category: "Unavailable".into(),
            description: "Тип ноди недоступний".into(),
            help: String::new(),
            pins,
        }
    }

    pub fn message(&self) -> String {
        match &self.plugin {
            Some(plugin) => format!("плагін {plugin} недоступний"),
            None => format!("тип ноди {} невідомий: потрібного плагіна немає або його вимкнено", self.definition.type_name),
        }
    }
}

impl Node for StubNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition { self.definition.clone() }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        ctx.host_state.log(self.id, format!("[ERROR] Нода не виконується: {}", self.message()));
        Vec::new()
    }

    fn properties(&self) -> Vec<(String, DataValue)> { self.properties.clone() }
    fn unavailable(&self) -> Option<String> { Some(self.message()) }
}

impl TickEngine {
    /// Піни заглушки для невідомого типу: відновлюються з з'єднань графа. Тип піна даних береться
    /// з іншого кінця з'єднання, а якщо там теж заглушка — `Any`.
    pub(crate) fn infer_stub_pins(&self, id: NodeId, graph: &GraphModel, map_id: &impl Fn(NodeId) -> NodeId) -> Vec<PinDefinition> {
        let mut pins: Vec<PinDefinition> = Vec::new();
        let mut add = |pin: PinId, direction: PinDirection, pin_type: PinType| {
            if !pins.iter().any(|p| p.id == pin && p.direction == direction) {
                pins.push(PinDefinition { id: pin, name: format!("#{}", pin.0), pin_type, direction, required: false });
            }
        };
        let other_type = |node: NodeId, pin: PinId, direction: PinDirection| {
            self.node_definition(node)
                .and_then(|d| d.pins.into_iter().find(|p| p.id == pin && p.direction == direction))
                .map_or(PinType::Data(DataType::Any), |p| p.pin_type)
        };
        for c in &graph.exec_connections {
            if map_id(c.src_node) == id {
                add(c.src_pin, PinDirection::Output, PinType::Exec);
            }
            if map_id(c.dst_node) == id {
                add(c.dst_pin, PinDirection::Input, PinType::Exec);
            }
        }
        for c in &graph.data_connections {
            if map_id(c.src_node) == id {
                add(c.src_pin, PinDirection::Output, other_type(map_id(c.dst_node), c.dst_pin, PinDirection::Input));
            }
            if map_id(c.dst_node) == id {
                add(c.dst_pin, PinDirection::Input, other_type(map_id(c.src_node), c.src_pin, PinDirection::Output));
            }
        }
        pins.sort_by_key(|p| (p.direction == PinDirection::Output, p.id.0));
        pins
    }

    /// Замінює ноди вказаних типів заглушками (плагін вимкнено). Повертає ID замінених нод.
    pub(crate) fn replace_with_stubs(&mut self, type_names: &HashSet<String>, plugin: &str) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self.nodes.iter()
            .filter(|(_, n)| type_names.contains(&n.definition().type_name))
            .map(|(id, _)| *id)
            .collect();
        ids.sort_by_key(|id| id.0);
        for id in &ids {
            let Some(model) = self.node_model(*id) else { continue };
            let definition = self.nodes[id].definition();
            self.host_state.cancel_node_tasks(*id);
            self.input_values.remove(id);
            self.required_reported.remove(id);
            self.data_cache.remove(id);
            self.pure_cache.remove(id);
            // Стара нода (WasmNode) звільняє свій ресурс у плагіні при заміні
            self.nodes.insert(*id, Box::new(StubNode::new(*id, definition, model.properties, Some(plugin.to_string()))));
        }
        self.pure_cache.clear();
        ids
    }

    /// Перетворює заглушки, чий тип знову зареєстровано, на справжні ноди. З'єднання, що стали
    /// невалідними (піни змінились), прибираються й повертаються.
    pub(crate) fn revive_stubs(&mut self) -> (Vec<NodeId>, RemovedConnections) {
        let mut ids: Vec<NodeId> = self.nodes.iter()
            .filter(|(_, n)| n.unavailable().is_some() && self.registry.get_definition(&n.definition().type_name).is_some())
            .map(|(id, _)| *id)
            .collect();
        ids.sort_by_key(|id| id.0);
        let mut revived = Vec::new();
        for id in ids {
            let Some(model) = self.node_model(id) else { continue };
            match self.create_node(id, &model.type_name, &model.properties) {
                Ok(()) => revived.push(id),
                Err(EngineError::UnknownNodeType(_)) => {}
                Err(e) => tracing::warn!(node = ?id, "Не вдалось відновити ноду {}: {e}", model.type_name),
            }
        }
        self.pure_cache.clear();
        let removed = self.prune_connections(|c| revived.contains(&c.src_node) || revived.contains(&c.dst_node));
        (revived, removed)
    }
}
