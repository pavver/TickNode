use std::collections::{BTreeMap, HashMap, VecDeque};
use ticknode_shared::*;
use ticknode_shared::condition::{MatchCase, MatchInput, match_inputs};
use tracing::info;
use thiserror::Error;
use evalexpr::*;

mod host;
mod plugin;
mod custom_types;
mod stub;

pub use host::*;
pub use plugin::*;
pub use custom_types::*;
pub use stub::StubNode;

pub struct NodeContext<'a> {
    pub node_id: NodeId,
    pub inputs: HashMap<PinId, Option<DataValue>>,
    pub variables: &'a HashMap<String, DataValue>,
    pub host_state: &'a mut HostState,
}

impl<'a> NodeContext<'a> {
    pub fn get_number(&self, pin: PinId, default: NumberValue) -> NumberValue {
        match self.inputs.get(&pin) {
            Some(Some(DataValue::Number(v))) => *v,
            _ => default,
        }
    }
    pub fn get_bool(&self, pin: PinId, default: bool) -> bool {
        match self.inputs.get(&pin) {
            Some(Some(DataValue::Bool(v))) => *v,
            _ => default,
        }
    }
    pub fn get_value(&self, pin: PinId) -> Option<DataValue> {
        self.inputs.get(&pin).cloned().flatten()
    }
    pub fn get_var(&self, name: &str) -> Option<DataValue> {
        self.variables.get(name).cloned()
    }
}

pub trait Node: Send + Sync {
    fn id(&self) -> NodeId;
    fn definition(&self) -> NodeDefinition;
    fn is_pure(&self) -> bool { false }
    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput>;
    fn set_property(&mut self, _name: &str, _value: DataValue) {}
    /// Властивості, через які ноду можна відтворити (збереження графа, редактор).
    fn properties(&self) -> Vec<(String, DataValue)> { Vec::new() }
    /// Ноду відтворено: властивості з `create_node` застосовано. Далі `set_property` — це вже
    /// правки, а не відновлення збереженого стану.
    fn restored(&mut self) {}
    /// Ноди-джерела подій (таймери, тригери): викликається на початку кожного такту.
    /// Повернений пін стає тригером виконання ноди в цьому ж такті.
    fn poll(&mut self, _host: &mut HostState) -> Option<PinId> { None }
    /// Завершилась задача, яку нода запустила через `HostState` (затримка, HTTP).
    /// Викликається в такті завершення, до виконання черги.
    fn on_task(&mut self, _task: TaskId, _result: TaskResult, _ctx: &mut NodeContext) -> Vec<NodeOutput> { Vec::new() }
    /// Нода-заглушка (плагін типу недоступний): пояснення для редактора. Така нода не виконується
    /// і не приймає змін властивостей, але зберігає піни й властивості для збереження графа.
    fn unavailable(&self) -> Option<String> { None }
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum EngineError {
    #[error("Циклічне посилання: нода {0:?} вже залежить від {1:?}")]
    CycleDetected(NodeId, NodeId),
    #[error("Несумісні типи даних: спроба з'єднати {0:?} з {1:?}")]
    TypeMismatch(DataType, DataType),
    #[error("Некоректне з'єднання пінів: один або обидва піни не існують або мають невірний напрямок/тип")]
    InvalidPinConnection,
    #[error("Ноду {0:?} не знайдено")]
    NodeNotFound(NodeId),
    #[error("Невідомий тип ноди: {0}")]
    UnknownNodeType(String),
    #[error("Нода недоступна: {0}; змінити її не можна")]
    PluginUnavailable(String),
}

pub type NodeCreator = Box<dyn Fn(NodeId) -> Box<dyn Node> + Send + Sync>;

pub struct NodeRegistry {
    definitions: HashMap<String, NodeDefinition>,
    creators: HashMap<String, NodeCreator>,
    /// Який плагін зареєстрував тип ноди (вбудовані типи власника не мають).
    owners: HashMap<String, String>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            definitions: HashMap::new(),
            creators: HashMap::new(),
            owners: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Реєструє тип ноди. Дефініція береться з екземпляра, створеного з дефолтними властивостями,
    /// тому реєстр і сама нода не можуть розійтись.
    pub fn register(&mut self, creator: NodeCreator) {
        let definition = creator(NodeId(uuid::Uuid::nil())).definition();
        self.register_definition(definition, creator);
    }

    /// Реєструє тип ноди від імені плагіна (щоб зняти його при вимкненні плагіна).
    pub fn register_owned(&mut self, owner: &str, creator: NodeCreator) {
        let definition = creator(NodeId(uuid::Uuid::nil())).definition();
        self.owners.insert(definition.type_name.clone(), owner.to_string());
        self.register_definition(definition, creator);
    }

    fn register_definition(&mut self, definition: NodeDefinition, creator: NodeCreator) {
        let type_name = definition.type_name.clone();
        self.definitions.insert(type_name.clone(), definition);
        self.creators.insert(type_name, creator);
    }

    pub fn owner_of(&self, type_name: &str) -> Option<&str> {
        self.owners.get(type_name).map(String::as_str)
    }

    pub fn types_owned_by(&self, owner: &str) -> Vec<String> {
        self.owners.iter().filter(|(_, o)| *o == owner).map(|(t, _)| t.clone()).collect()
    }

    /// Знімає всі типи нод плагіна; повертає їхні назви.
    pub fn unregister_owner(&mut self, owner: &str) -> Vec<String> {
        let names: Vec<String> = self.owners.iter().filter(|(_, o)| *o == owner).map(|(t, _)| t.clone()).collect();
        for name in &names {
            self.owners.remove(name);
            self.definitions.remove(name);
            self.creators.remove(name);
        }
        names
    }

    pub fn get_definitions(&self) -> Vec<NodeDefinition> {
        let mut defs: Vec<_> = self.definitions.values().cloned().collect();
        defs.sort_by(|a, b| (&a.category, &a.title).cmp(&(&b.category, &b.title)));
        defs
    }

    pub fn get_definition(&self, type_name: &str) -> Option<&NodeDefinition> {
        self.definitions.get(type_name)
    }

    pub fn create_node(&self, type_name: &str, id: NodeId) -> Option<Box<dyn Node>> {
        self.creators.get(type_name).map(|f| f(id))
    }

    fn register_defaults(&mut self) {
        self.register(Box::new(|id| Box::new(ClockNode { id, interval_ticks: 10 })));
        self.register(Box::new(|id| Box::new(ConsoleNode { id })));
        self.register(Box::new(|id| Box::new(ConstNode { id, value: DataValue::Number(NumberValue::Int(0)) })));
        for op in [MathOp::Add, MathOp::Sub, MathOp::Mul, MathOp::Div] {
            self.register(Box::new(move |id| Box::new(PureMathNode { id, op })));
        }
        self.register(Box::new(|id| Box::new(MathExpressionNode {
            id,
            expression: "a + b".to_string(),
            variable_mapping: expression_variables("a + b"),
        })));
        for op in [LogOp::And, LogOp::Or, LogOp::Not] {
            self.register(Box::new(move |id| Box::new(LogicalNode { id, op })));
        }
        self.register(Box::new(|id| Box::new(VariableGetNode { id, var_name: "my_var".to_string() })));
        self.register(Box::new(|id| Box::new(VariableSetNode { id, var_name: "my_var".to_string() })));
        self.register(Box::new(|id| Box::new(MatchNode::new(id))));
        self.register(Box::new(|id| Box::new(CompareNode { id })));
        self.register(Box::new(|id| Box::new(ForLoopNode { id, active: false, current_index: 0, count: 0 })));
        self.register(Box::new(|id| Box::new(SleepNode { id })));
        self.register(Box::new(|id| Box::new(ListCreateNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListAddNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListRemoveNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListGetNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListLengthNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ObjectCreateNode { id })));
        self.register(Box::new(|id| Box::new(ObjectSetFieldNode { id, field_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ObjectGetFieldNode { id, field_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ObjectUnpackNode { id, fields_list: String::new(), parsed_fields: Vec::new() })));
        self.register(Box::new(|id| Box::new(ClickButtonNode { id, label: "Button".to_string(), clicked: false })));
        self.register(Box::new(|id| Box::new(DebugNode { id, prefix: String::new() })));
    }
}

impl Default for NodeRegistry {
    fn default() -> Self { Self::new() }
}

/// З'єднання, прибрані рушієм автоматично (напр. після зміни пінів ноди).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct RemovedConnections {
    pub exec: Vec<ConnectionModel>,
    pub data: Vec<ConnectionModel>,
}

pub struct TickEngine {
    nodes: HashMap<NodeId, Box<dyn Node>>,
    exec_connections: Vec<ConnectionModel>,
    data_connections: Vec<ConnectionModel>,
    pub registry: NodeRegistry,
    /// Власні типи даних, оголошені плагінами.
    pub types: TypeRegistry,
    /// Який плагін оголосив власний тип даних.
    type_owners: HashMap<String, String>,
    /// Завантажені плагіни (увімкнені й ні); див. `plugin.rs`.
    plugins: std::collections::BTreeMap<String, plugin::PluginSlot>,
    pub data_cache: HashMap<NodeId, HashMap<PinId, DataValue>>,
    pub pure_cache: HashMap<NodeId, HashMap<PinId, DataValue>>,
    pub variables: HashMap<String, DataValue>,
    /// Значення непід'єднаних входів, задані в редакторі (властивості `in:<пін>`).
    input_values: HashMap<NodeId, BTreeMap<u32, DataValue>>,
    /// Ноди, для яких помилку «не задано обов'язковий вхід» уже записано в лог (щоб не повторювати щотакту).
    required_reported: std::collections::HashSet<NodeId>,
    pub execution_queue: VecDeque<(NodeId, PinId)>,
    // Спільний стан асинхронних задач та дескрипторів
    pub host_state: HostState,
    /// Ноди, що виконались за останній такт, з входами й виходами (телеметрія для редактора).
    /// Чисті ноди потрапляють сюди, коли їх обчислили на запит.
    pub executed: Vec<NodeTrace>,
}

impl Default for TickEngine {
    fn default() -> Self { Self::new() }
}

impl TickEngine {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            exec_connections: Vec::new(),
            data_connections: Vec::new(),
            registry: NodeRegistry::new(),
            types: TypeRegistry::new(),
            type_owners: HashMap::new(),
            plugins: Default::default(),
            data_cache: HashMap::new(),
            pure_cache: HashMap::new(),
            variables: HashMap::new(),
            input_values: HashMap::new(),
            required_reported: Default::default(),
            execution_queue: VecDeque::new(),
            host_state: HostState::default(),
            executed: Vec::new(),
        }
    }

    // --- Ноди ---

    pub fn add_node(&mut self, node: Box<dyn Node>) {
        self.nodes.insert(node.id(), node);
    }

    /// Створює ноду через реєстр і застосовує властивості. Значення входів (`in:<пін>`) повертаються
    /// окремо: їх можна задати лише після властивостей, бо піни залежать від них.
    fn build_node(&self, id: NodeId, type_name: &str, properties: &[(String, DataValue)]) -> Result<(Box<dyn Node>, Vec<(PinId, DataValue)>), EngineError> {
        let mut node = self.registry.create_node(type_name, id)
            .ok_or_else(|| EngineError::UnknownNodeType(type_name.to_string()))?;
        let mut inputs = Vec::new();
        for (name, value) in properties {
            match parse_input_value_property(name) {
                Some(pin) => inputs.push((pin, value.clone())),
                None => node.set_property(name, value.clone()),
            }
        }
        node.restored();
        Ok((node, inputs))
    }

    /// Створює ноду через реєстр і застосовує властивості.
    pub fn create_node(&mut self, id: NodeId, type_name: &str, properties: &[(String, DataValue)]) -> Result<(), EngineError> {
        let (node, inputs) = self.build_node(id, type_name, properties)?;
        self.add_node(node);
        // Піни залежать від властивостей, тож значення входів — після них
        for (pin, value) in inputs {
            let _ = self.set_input_value(id, pin, Some(value));
        }
        Ok(())
    }

    /// Чому нода недоступна (заглушка); `None` — нода справжня.
    pub fn node_unavailable(&self, node_id: NodeId) -> Option<String> {
        self.nodes.get(&node_id)?.unavailable()
    }

    pub fn has_node(&self, node_id: NodeId) -> bool {
        self.nodes.contains_key(&node_id)
    }

    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().copied().collect()
    }

    pub fn node_definition(&self, node_id: NodeId) -> Option<NodeDefinition> {
        self.nodes.get(&node_id).map(|n| n.definition())
    }

    /// Модель ноди для збереження/редактора (позиція — справа сервера, тут за замовчуванням).
    pub fn node_model(&self, node_id: NodeId) -> Option<NodeModel> {
        self.nodes.get(&node_id).map(|n| NodeModel {
            id: node_id,
            type_name: n.definition().type_name,
            properties: n.properties().into_iter()
                .chain(self.input_values.get(&node_id).into_iter().flatten()
                    .map(|(pin, v)| (input_value_property(PinId(*pin)), v.clone())))
                .collect(),
            position: Position::default(),
        })
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> RemovedConnections {
        self.nodes.remove(&node_id);
        self.input_values.remove(&node_id);
        self.required_reported.remove(&node_id);
        self.data_cache.remove(&node_id);
        self.pure_cache.remove(&node_id);
        self.execution_queue.retain(|(n, _)| *n != node_id);

        self.host_state.cancel_node_tasks(node_id);

        let mut removed = RemovedConnections::default();
        self.exec_connections.retain(|c| {
            let keep = c.src_node != node_id && c.dst_node != node_id;
            if !keep { removed.exec.push(*c); }
            keep
        });
        self.data_connections.retain(|c| {
            let keep = c.src_node != node_id && c.dst_node != node_id;
            if !keep { removed.data.push(*c); }
            keep
        });
        removed
    }

    /// Змінює властивість ноди. Якщо через це змінились піни, невалідні з'єднання прибираються.
    pub fn set_node_property(&mut self, node_id: NodeId, name: &str, value: DataValue) -> Result<RemovedConnections, EngineError> {
        if let Some(pin) = parse_input_value_property(name) {
            self.set_input_value(node_id, pin, Some(value))?;
            return Ok(RemovedConnections::default());
        }
        let node = self.nodes.get_mut(&node_id).ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        node.set_property(name, value);
        self.pure_cache.remove(&node_id);
        self.data_cache.remove(&node_id);

        // Значення входів, що зникли чи змінили тип, більше не діють
        let definition = node.definition();
        if let Some(values) = self.input_values.get_mut(&node_id) {
            values.retain(|pin, v| input_pin_type(&definition, PinId(*pin)).is_some_and(|t| input_value_fits(&self.types, &t, v)));
        }

        Ok(self.prune_connections(|c| c.src_node == node_id || c.dst_node == node_id))
    }

    /// Прибирає з'єднання, що торкаються вибраних нод і стали невалідними (змінились піни).
    fn prune_connections(&mut self, touching: impl Fn(&ConnectionModel) -> bool) -> RemovedConnections {
        let invalid_exec: Vec<_> = self.exec_connections.iter()
            .filter(|c| touching(c) && self.validate_exec(c).is_err())
            .copied().collect();
        let invalid_data: Vec<_> = self.data_connections.iter()
            .filter(|c| touching(c) && self.validate_data_types(c).is_err())
            .copied().collect();
        self.exec_connections.retain(|c| !invalid_exec.contains(c));
        self.data_connections.retain(|c| !invalid_data.contains(c));
        RemovedConnections { exec: invalid_exec, data: invalid_data }
    }

    /// Значення непід'єднаного входу даних (`None` — прибрати). Діє, поки до піна нічого не під'єднано.
    pub fn set_input_value(&mut self, node_id: NodeId, pin: PinId, value: Option<DataValue>) -> Result<(), EngineError> {
        let node = self.nodes.get(&node_id).ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        let values = self.input_values.entry(node_id).or_default();
        match value {
            Some(value) => {
                let pin_type = input_pin_type(&node.definition(), pin).ok_or(EngineError::InvalidPinConnection)?;
                if !input_value_fits(&self.types, &pin_type, &value) {
                    return Err(EngineError::TypeMismatch(value.get_type(), pin_type));
                }
                values.insert(pin.0, value);
            }
            None => {
                values.remove(&pin.0);
            }
        }
        self.pure_cache.clear();
        Ok(())
    }

    // --- З'єднання ---

    pub fn exec_connections(&self) -> &[ConnectionModel] {
        &self.exec_connections
    }

    pub fn data_connections(&self) -> &[ConnectionModel] {
        &self.data_connections
    }

    fn find_pin(&self, node_id: NodeId, pin_id: PinId, direction: PinDirection) -> Result<PinType, EngineError> {
        let node = self.nodes.get(&node_id).ok_or(EngineError::NodeNotFound(node_id))?;
        node.definition().pins.into_iter()
            .find(|p| p.id == pin_id && p.direction == direction)
            .map(|p| p.pin_type)
            .ok_or(EngineError::InvalidPinConnection)
    }

    fn validate_exec(&self, c: &ConnectionModel) -> Result<(), EngineError> {
        match (self.find_pin(c.src_node, c.src_pin, PinDirection::Output)?, self.find_pin(c.dst_node, c.dst_pin, PinDirection::Input)?) {
            (PinType::Exec, PinType::Exec) => Ok(()),
            _ => Err(EngineError::InvalidPinConnection),
        }
    }

    fn validate_data_types(&self, c: &ConnectionModel) -> Result<(), EngineError> {
        match (self.find_pin(c.src_node, c.src_pin, PinDirection::Output)?, self.find_pin(c.dst_node, c.dst_pin, PinDirection::Input)?) {
            (PinType::Data(s_type), PinType::Data(d_type)) => {
                if self.types.compatible(&s_type, &d_type) {
                    Ok(())
                } else {
                    Err(EngineError::TypeMismatch(s_type, d_type))
                }
            }
            _ => Err(EngineError::InvalidPinConnection),
        }
    }

    pub fn connect_exec(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> Result<(), EngineError> {
        let c = ConnectionModel { src_node, src_pin, dst_node, dst_pin };
        self.validate_exec(&c)?;
        if !self.exec_connections.contains(&c) {
            self.exec_connections.push(c);
        }
        Ok(())
    }

    pub fn disconnect_exec(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> bool {
        let c = ConnectionModel { src_node, src_pin, dst_node, dst_pin };
        let before = self.exec_connections.len();
        self.exec_connections.retain(|x| *x != c);
        before != self.exec_connections.len()
    }

    /// Чи залежить `start` (через вхідні піни даних) від `target`.
    fn has_data_path(&self, start: NodeId, target: NodeId) -> bool {
        let mut stack = vec![start];
        let mut visited = std::collections::HashSet::new();
        while let Some(current) = stack.pop() {
            if current == target { return true; }
            if !visited.insert(current) { continue; }
            for c in &self.data_connections {
                if c.dst_node == current { stack.push(c.src_node); }
            }
        }
        false
    }

    /// Вхідний пін даних має рівно одне джерело: нове з'єднання замінює попереднє.
    pub fn connect_data(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> Result<(), EngineError> {
        let c = ConnectionModel { src_node, src_pin, dst_node, dst_pin };
        self.validate_data_types(&c)?;
        if self.has_data_path(src_node, dst_node) {
            return Err(EngineError::CycleDetected(dst_node, src_node));
        }
        self.data_connections.retain(|x| x.dst_node != dst_node || x.dst_pin != dst_pin);
        self.data_connections.push(c);
        self.pure_cache.clear();
        Ok(())
    }

    pub fn disconnect_data(&mut self, dst_node: NodeId, dst_pin: PinId) -> bool {
        let before = self.data_connections.len();
        self.data_connections.retain(|c| c.dst_node != dst_node || c.dst_pin != dst_pin);
        self.pure_cache.clear();
        before != self.data_connections.len()
    }

    pub fn trigger_node(&mut self, node_id: NodeId, pin_id: PinId) {
        self.execution_queue.push_back((node_id, pin_id));
    }

    // --- Граф цілком ---

    /// Експорт у стабільному порядку (зручно для diff у git).
    pub fn export_graph(&self) -> GraphModel {
        let mut ids = self.node_ids();
        ids.sort_by_key(|id| id.0);
        self.export_snippet(&ids)
    }

    pub fn export_snippet(&self, node_ids: &[NodeId]) -> GraphModel {
        let nodes = node_ids.iter().filter_map(|id| self.node_model(*id)).collect();
        let sort_key = |c: &ConnectionModel| (c.src_node.0, c.src_pin.0, c.dst_node.0, c.dst_pin.0);
        let mut exec_connections: Vec<_> = self.exec_connections.iter()
            .filter(|c| node_ids.contains(&c.src_node) && node_ids.contains(&c.dst_node))
            .copied().collect();
        exec_connections.sort_by_key(sort_key);
        let mut data_connections: Vec<_> = self.data_connections.iter()
            .filter(|c| node_ids.contains(&c.src_node) && node_ids.contains(&c.dst_node))
            .copied().collect();
        data_connections.sort_by_key(sort_key);
        GraphModel { nodes, exec_connections, data_connections }
    }

    /// Замінює поточний граф збереженим. Повертає попередження про елементи, які не вдалось відновити.
    pub fn load_graph(&mut self, graph: &GraphModel) -> Vec<String> {
        for id in self.node_ids() {
            self.remove_node(id);
        }
        self.execution_queue.clear();
        self.variables.clear();
        self.insert_graph(graph, |id| id)
    }

    /// Вставляє фрагмент графа з новими ID (копіювання/вставка).
    /// Повертає пари (ID у фрагменті, новий ID) для створених нод і попередження.
    /// З'єднання з нодами поза фрагментом ігноруються.
    pub fn import_snippet(&mut self, snippet: &GraphModel) -> (Vec<(NodeId, NodeId)>, Vec<String>) {
        let id_map: HashMap<NodeId, NodeId> = snippet.nodes.iter().map(|n| (n.id, NodeId::generate())).collect();
        let inside = |c: &&ConnectionModel| id_map.contains_key(&c.src_node) && id_map.contains_key(&c.dst_node);
        let snippet = GraphModel {
            nodes: snippet.nodes.clone(),
            exec_connections: snippet.exec_connections.iter().filter(inside).copied().collect(),
            data_connections: snippet.data_connections.iter().filter(inside).copied().collect(),
        };
        let warnings = self.insert_graph(&snippet, |id| id_map[&id]);
        let created = snippet.nodes.iter().map(|n| (n.id, id_map[&n.id])).filter(|(_, new)| self.has_node(*new)).collect();
        (created, warnings)
    }

    fn insert_graph(&mut self, graph: &GraphModel, map_id: impl Fn(NodeId) -> NodeId) -> Vec<String> {
        let mut warnings = Vec::new();
        let mut unknown = Vec::new();
        for n in &graph.nodes {
            match self.create_node(map_id(n.id), &n.type_name, &n.properties) {
                Ok(()) => {}
                Err(EngineError::UnknownNodeType(_)) => unknown.push(n),
                Err(e) => warnings.push(format!("Нода {:?}: {}", n.id, e)),
            }
        }
        // Невідомий тип (плагін вимкнено чи не завантажено) не губить ноду: заглушка зберігає
        // властивості й з'єднання, тож граф запишеться назад без втрат
        for n in unknown {
            let id = map_id(n.id);
            let pins = self.infer_stub_pins(id, graph, &map_id);
            tracing::warn!(node = ?n.id, "Невідомий тип ноди {}: створено заглушку", n.type_name);
            self.add_node(Box::new(StubNode::new(id, StubNode::bare_definition(&n.type_name, pins), n.properties.clone(), None)));
        }
        for c in &graph.exec_connections {
            if let Err(e) = self.connect_exec(map_id(c.src_node), c.src_pin, map_id(c.dst_node), c.dst_pin) {
                warnings.push(format!("Exec-з'єднання {:?}: {}", c, e));
            }
        }
        for c in &graph.data_connections {
            if let Err(e) = self.connect_data(map_id(c.src_node), c.src_pin, map_id(c.dst_node), c.dst_pin) {
                warnings.push(format!("З'єднання даних {:?}: {}", c, e));
            }
        }
        warnings
    }

    // --- Виконання ---

    fn pull_data(&mut self, node_id: NodeId, pin_id: PinId, stack: &mut Vec<NodeId>) -> Option<DataValue> {
        if stack.contains(&node_id) { return None; }

        // Перевіримо, чи це вхідний пін, з'єднаний з виходом іншої ноди.
        // Якщо так, рекурсивно тягнемо дані з джерела.
        let incoming = self.data_connections.iter()
            .find(|c| c.dst_node == node_id && c.dst_pin == pin_id)
            .copied();
        if let Some(c) = incoming {
            stack.push(node_id);
            let val = self.pull_data(c.src_node, c.src_pin, stack);
            stack.pop();
            return val;
        }

        if let Some(cached_node_vals) = self.pure_cache.get(&node_id) {
            if let Some(val) = cached_node_vals.get(&pin_id) {
                return Some(val.clone());
            }
        }

        let is_pure = self.nodes.get(&node_id).map(|n| n.is_pure()).unwrap_or(false);
        if is_pure {
            stack.push(node_id);
            let mut node = self.nodes.remove(&node_id)?;
            let inputs = self.pull_inputs(node_id, stack);
            if self.skip_for_required(node_id, &node.definition(), &inputs) {
                self.nodes.insert(node_id, node);
                stack.pop();
                return None;
            }
            let traced = trace_inputs(&inputs);
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let outputs = node.execute(None, &mut ctx);
            self.nodes.insert(node_id, node);
            stack.pop();

            let mut node_outputs = HashMap::new();
            let mut target_val = None;
            for out in outputs {
                if let NodeOutput::Data(p, v) = out {
                    if p == pin_id {
                        target_val = Some(v.clone());
                    }
                    node_outputs.insert(p, v);
                }
            }
            self.pure_cache.entry(node_id).or_default().extend(node_outputs);
            self.record_trace(node_id, traced, Vec::new());
            target_val
        } else {
            self.data_cache.get(&node_id).and_then(|m| m.get(&pin_id)).cloned()
        }
    }

    fn pull_inputs(&mut self, node_id: NodeId, stack: &mut Vec<NodeId>) -> HashMap<PinId, Option<DataValue>> {
        let connections: Vec<_> = self.data_connections.iter().filter(|c| c.dst_node == node_id).copied().collect();
        let mut inputs: HashMap<PinId, Option<DataValue>> = connections.into_iter()
            .map(|c| (c.dst_pin, self.pull_data(c.src_node, c.src_pin, stack)))
            .collect();
        for (pin, value) in self.input_values.get(&node_id).into_iter().flatten() {
            inputs.entry(PinId(*pin)).or_insert_with(|| Some(value.clone()));
        }
        inputs
    }

    /// Нода з незаданим обов'язковим входом не виконується. Вхід заданий, якщо до нього під'єднано
    /// джерело або на ноді задано значення. Джерело без значення — ноду пропускаємо мовчки,
    /// відсутність входу — помилка в лозі (один раз, доки стан не зміниться).
    fn skip_for_required(&mut self, node_id: NodeId, def: &NodeDefinition, inputs: &HashMap<PinId, Option<DataValue>>) -> bool {
        let mut pending = false;
        for pin in def.pins.iter().filter(|p| p.required && p.direction == PinDirection::Input && p.pin_type != PinType::Exec) {
            match inputs.get(&pin.id) {
                Some(Some(_)) => {}
                Some(None) => pending = true,
                None => {
                    if self.required_reported.insert(node_id) {
                        self.host_state.log(node_id, format!("[ERROR] Не задано обов'язковий вхід «{}»", pin.name));
                    }
                    return true;
                }
            }
        }
        if !pending {
            self.required_reported.remove(&node_id);
        }
        pending
    }

    pub fn tick(&mut self) {
        self.pure_cache.clear();
        self.executed.clear();
        self.host_state.current_tick += 1;
        let mut next_tick_queue = VecDeque::new();

        // Завершені задачі (затримки, HTTP) доставляються нодам-власникам.
        for (node_id, task, result) in self.host_state.take_finished() {
            if !self.nodes.contains_key(&node_id) { continue; }
            let mut stack = Vec::new();
            let inputs = self.pull_inputs(node_id, &mut stack);
            let traced = trace_inputs(&inputs);
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let Some(node) = self.nodes.get_mut(&node_id) else { continue };
            let outputs = node.on_task(task, result, &mut ctx);
            self.apply_outputs(node_id, traced, outputs, &mut next_tick_queue);
        }

        let mut fired = Vec::new();
        for (id, node) in self.nodes.iter_mut() {
            if let Some(pin) = node.poll(&mut self.host_state) {
                fired.push((*id, pin));
            }
        }
        for (id, pin) in fired {
            self.trigger_node(id, pin);
        }

        let mut current_tick_queue = std::mem::take(&mut self.execution_queue);
        while let Some((node_id, trigger_pin)) = current_tick_queue.pop_front() {
            if !self.nodes.contains_key(&node_id) { continue; }
            let mut stack = Vec::new();
            let inputs = self.pull_inputs(node_id, &mut stack);
            if let Some(def) = self.node_definition(node_id) && self.skip_for_required(node_id, &def, &inputs) {
                continue;
            }
            let traced = trace_inputs(&inputs);
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let Some(node) = self.nodes.get_mut(&node_id) else { continue };
            let outputs = node.execute(Some(trigger_pin), &mut ctx);
            self.apply_outputs(node_id, traced, outputs, &mut next_tick_queue);
        }
        self.execution_queue.extend(next_tick_queue);
    }

    /// Exec-виходи стають у чергу наступного такту; дані й змінні застосовуються одразу.
    fn apply_outputs(
        &mut self,
        node_id: NodeId,
        inputs: Vec<(PinId, Option<DataValue>)>,
        outputs: Vec<NodeOutput>,
        next_tick_queue: &mut VecDeque<(NodeId, PinId)>,
    ) {
        let mut state_changed = false;
        let mut fired = Vec::new();
        for output in outputs {
            match output {
                NodeOutput::Execution(out_exec_pin) => {
                    fired.push(out_exec_pin);
                    for c in &self.exec_connections {
                        if c.src_node == node_id && c.src_pin == out_exec_pin {
                            next_tick_queue.push_back((c.dst_node, c.dst_pin));
                        }
                    }
                }
                NodeOutput::Data(pin_id, value) => {
                    self.data_cache.entry(node_id).or_default().insert(pin_id, value);
                    state_changed = true;
                }
                NodeOutput::Variable(name, value) => {
                    self.variables.insert(name, value);
                    state_changed = true;
                }
            }
        }
        // Чисті ноди, обчислені раніше в цьому такті, могли залежати від змінених даних.
        if state_changed {
            self.pure_cache.clear();
        }
        self.record_trace(node_id, inputs, fired);
    }

    /// Нода виконалась кілька разів за такт — лишається останнє виконання, exec-виходи накопичуються.
    fn record_trace(&mut self, node_id: NodeId, inputs: Vec<(PinId, Option<DataValue>)>, fired: Vec<PinId>) {
        let mut outputs: Vec<(PinId, DataValue)> = self.data_cache.get(&node_id).into_iter().flatten()
            .chain(self.pure_cache.get(&node_id).into_iter().flatten())
            .map(|(p, v)| (*p, v.clone()))
            .collect();
        outputs.sort_by_key(|(p, _)| p.0);
        outputs.dedup_by_key(|(p, _)| *p);
        let tick = self.host_state.current_tick;
        match self.executed.iter_mut().find(|t| t.node_id == node_id) {
            Some(trace) => {
                trace.inputs = inputs;
                trace.outputs = outputs;
                for pin in fired {
                    if !trace.fired.contains(&pin) {
                        trace.fired.push(pin);
                    }
                }
            }
            None => self.executed.push(NodeTrace { node_id, tick, inputs, outputs, fired }),
        }
    }
}

/// Тип вхідного піна даних (`None` — немає такого або це exec-пін).
fn input_pin_type(definition: &NodeDefinition, pin: PinId) -> Option<DataType> {
    definition.pins.iter()
        .find(|p| p.id == pin && p.direction == PinDirection::Input)
        .and_then(|p| match &p.pin_type {
            PinType::Data(t) => Some(t.clone()),
            PinType::Exec => None,
        })
}

fn trace_inputs(inputs: &HashMap<PinId, Option<DataValue>>) -> Vec<(PinId, Option<DataValue>)> {
    let mut traced: Vec<_> = inputs.iter().map(|(p, v)| (*p, v.clone())).collect();
    traced.sort_by_key(|(p, _)| p.0);
    traced
}

// --- РЕАЛІЗАЦІЯ ВБУДОВАНИХ НОД ---

pub struct ConstNode { pub id: NodeId, pub value: DataValue }
impl Node for ConstNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "const".to_string(),
            title: "Constant".to_string(),
            category: "Data".to_string(),
            description: "Повертає константне значення".to_string(),
            help: "Видає на вихід Val значення, задане прямо на ноді (рядок «Значення») або в правій панелі. Тип значення (число, рядок, логічне) обирається в правій панелі, і від нього залежить тип піна.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Val".to_string(),
                pin_type: PinType::Data(self.value.get_type()),
                direction: PinDirection::Output, required: false }],
        }
    }
    fn execute(&mut self, _:Option<PinId>, _:&mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(PinId(0), self.value.clone())]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("value".to_string(), self.value.clone())]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "value" {
            self.value = value;
        }
    }
}

pub struct PureMathNode { pub id: NodeId, pub op: MathOp }
impl Node for PureMathNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let name = match self.op {
            MathOp::Add => "math/add", MathOp::Sub => "math/sub", MathOp::Mul => "math/mul", MathOp::Div => "math/div",
        };
        let title = match self.op {
            MathOp::Add => "Math Add", MathOp::Sub => "Math Subtract", MathOp::Mul => "Math Multiply", MathOp::Div => "Math Divide",
        };
        NodeDefinition {
            type_name: name.to_string(),
            title: title.to_string(),
            category: "Math".to_string(),
            description: "Математична операція над двома числами".to_string(),
            help: match self.op {
                MathOp::Add => "Res = A + B.",
                MathOp::Sub => "Res = A − B.",
                MathOp::Mul => "Res = A × B.",
                MathOp::Div => "Res = A ÷ B. Ділення на 0 дає нескінченність.",
            }.to_string() + "\n\nНепід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається нулем. Ціле число лишається цілим, якщо результат цілий.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.",
            pins: vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "B".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Res".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a = ctx.get_number(PinId(0), NumberValue::Int(0));
        let b = ctx.get_number(PinId(1), NumberValue::Int(0));
        let res = match self.op {
            MathOp::Add => a + b, MathOp::Sub => a - b, MathOp::Mul => a * b, MathOp::Div => a / b,
        };
        vec![NodeOutput::Data(PinId(2), DataValue::Number(res))]
    }
}

pub struct MathExpressionNode {
    pub id: NodeId,
    pub expression: String,
    pub variable_mapping: HashMap<String, PinId>,
}
impl Node for MathExpressionNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![PinDefinition {
            id: PinId(0),
            name: "Res".to_string(),
            pin_type: PinType::Data(DataType::Number),
            direction: PinDirection::Output, required: false }];
        let mut vars: Vec<_> = self.variable_mapping.iter().collect();
        vars.sort_by_key(|(_, pin)| pin.0);
        for (name, pin_id) in vars {
            pins.push(PinDefinition {
                id: *pin_id,
                name: name.clone(),
                pin_type: PinType::Data(DataType::Number),
                direction: PinDirection::Input, required: false });
        }
        NodeDefinition {
            type_name: "math/expression".to_string(),
            title: "Expression".to_string(),
            category: "Math".to_string(),
            description: "Обчислює довільну математичну формулу".to_string(),
            help: "Обчислює формулу, задану прямо на ноді (рядок «Формула»), наприклад `(a + b) / 2` або `x * x - 1`.\n\nКожна змінна у формулі автоматично стає вхідним піном (в абетковому порядку). Непід'єднана змінна бере значення, задане біля піна, а без нього дорівнює 0. Якщо формула некоректна — результат 0.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins,
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let mut context = HashMapContext::new();
        for (name, pin) in &self.variable_mapping {
            let val = ctx.get_number(*pin, NumberValue::Int(0));
            let _ = context.set_value(name.clone().into(), Value::from(val.to_f64()));
        }
        let result = match eval_with_context(&self.expression, &context) {
            Ok(val) => {
                if let Ok(f) = val.as_float() { NumberValue::Float(f).normalize() }
                else if let Ok(i) = val.as_int() { NumberValue::Int(i).normalize() }
                else { NumberValue::Int(0) }
            }
            Err(_) => NumberValue::Int(0),
        };
        vec![NodeOutput::Data(PinId(0), DataValue::Number(result))]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("expression".to_string(), DataValue::String(self.expression.clone()))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "expression" {
            if let DataValue::String(s) = value {
                self.variable_mapping = expression_variables(&s);
                self.expression = s;
            }
        }
    }
}

/// Змінні формули стають вхідними пінами (1, 2, ...) у алфавітному порядку.
pub fn expression_variables(expression: &str) -> HashMap<String, PinId> {
    let mut names: Vec<String> = match build_operator_tree(expression) {
        Ok(tree) => tree.iter_read_variable_identifiers().map(str::to_string).collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names.dedup();
    names.into_iter().enumerate().map(|(i, n)| (n, PinId(i as u32 + 1))).collect()
}

pub struct LogicalNode { pub id: NodeId, pub op: LogOp }
impl Node for LogicalNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let name = match self.op {
            LogOp::And => "logical/and", LogOp::Or => "logical/or", LogOp::Not => "logical/not",
        };
        let title = match self.op {
            LogOp::And => "Logical And", LogOp::Or => "Logical Or", LogOp::Not => "Logical Not",
        };
        let pins = match self.op {
            LogOp::And | LogOp::Or => vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "B".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Res".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
            LogOp::Not => vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Res".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
        };
        NodeDefinition {
            type_name: name.to_string(),
            title: title.to_string(),
            category: "Logic".to_string(),
            description: "Логічна операція".to_string(),
            help: match self.op {
                LogOp::And => "Res = true, лише якщо і A, і B істинні.",
                LogOp::Or => "Res = true, якщо істинний хоча б один із A, B.",
                LogOp::Not => "Res — протилежне значення до A.",
            }.to_string() + "\n\nНепід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається false.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.",
            pins,
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a = ctx.get_bool(PinId(0), false);
        let b = ctx.get_bool(PinId(1), false);
        let res = match self.op { LogOp::And => a && b, LogOp::Or => a || b, LogOp::Not => !a };
        vec![NodeOutput::Data(PinId(2), DataValue::Bool(res))]
    }
}

pub struct VariableGetNode { pub id: NodeId, pub var_name: String }
impl Node for VariableGetNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "variable/get".to_string(),
            title: format!("Get {}", self.var_name),
            category: "Data".to_string(),
            description: "Повертає значення змінної за її назвою".to_string(),
            help: "Читає глобальну змінну з назвою з властивості «Змінна» і видає її на Val. Змінні спільні для всього графа, записує їх нода Set. Поки змінну не записано — видає 0.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Val".to_string(),
                pin_type: PinType::Data(DataType::Number),
                direction: PinDirection::Output, required: false }],
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_var(&self.var_name).unwrap_or(DataValue::Number(NumberValue::Int(0)));
        vec![NodeOutput::Data(PinId(0), val)]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("var_name".to_string(), DataValue::String(self.var_name.clone()))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "var_name" {
            if let DataValue::String(s) = value {
                self.var_name = s;
            }
        }
    }
}

pub struct VariableSetNode { pub id: NodeId, pub var_name: String }
impl Node for VariableSetNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "variable/set".to_string(),
            title: format!("Set {}", self.var_name),
            category: "Data".to_string(),
            description: "Записує значення у змінну".to_string(),
            help: "За сигналом на In записує значення з Val у глобальну змінну (назва — у властивості «Змінна») і передає виконання на Out.\n\nЗмінну бачать усі ноди Get з тією ж назвою в усьому графі.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(0), name: "Out".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(1), name: "Val".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
            ],
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_value(PinId(1)).unwrap_or(DataValue::Number(NumberValue::Int(0)));
        vec![NodeOutput::Execution(PinId(0)), NodeOutput::Variable(self.var_name.clone(), val)]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("var_name".to_string(), DataValue::String(self.var_name.clone()))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "var_name" {
            if let DataValue::String(s) = value {
                self.var_name = s;
            }
        }
    }
}

/// Комбінатор-вирішувач: за сигналом на In перевіряє випадки й передає виконання на вихід
/// першого, що збігся (або всіх, що збіглись), інакше — на Else.
/// Входи — явний список (`inputs`: пін, ім'я, тип); у старих графах без нього — імена з умов.
pub struct MatchNode {
    pub id: NodeId,
    pub cases: Vec<MatchCase>,
    /// Спрацьовують усі випадки, що збіглись, а не лише перший.
    pub all_matches: bool,
    inputs: Vec<MatchInput>,
    /// Входи задані явно (інакше — виводяться з умов, як у старих графах).
    explicit_inputs: bool,
}

impl MatchNode {
    pub const ELSE_PIN: PinId = PinId(0);

    pub fn new(id: NodeId) -> Self {
        // Типовий вхід `a` виводиться з умови. Поки нода відновлюється, випадки без `inputs`
        // (старий формат) виводять входи; після `restored` входи явні — і в новій ноді, і в завантаженій.
        let mut node = Self { id, cases: Vec::new(), all_matches: false, inputs: Vec::new(), explicit_inputs: false };
        node.set_cases(vec![MatchCase::parse("1: a > 0").expect("типовий випадок коректний")]);
        node
    }

    /// Номери пінів випадків унікальні й не збігаються з Else.
    fn set_cases(&mut self, mut cases: Vec<MatchCase>) {
        let mut seen = std::collections::HashSet::new();
        cases.retain(|c| c.pin != Self::ELSE_PIN.0 && seen.insert(c.pin));
        self.cases = cases;
        if !self.explicit_inputs {
            self.inputs = match_inputs(&self.cases).into_iter().enumerate()
                .map(|(i, name)| MatchInput { pin: i as u32 + 1, name, value_type: DataType::Any })
                .collect();
        }
    }

    /// Піни й імена входів унікальні; пін 0 зайнятий exec-входом In.
    fn set_inputs(&mut self, mut inputs: Vec<MatchInput>) {
        let mut pins = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        inputs.retain(|i| i.pin != 0 && pins.insert(i.pin) && names.insert(i.name.clone()));
        self.inputs = inputs;
        self.explicit_inputs = true;
    }
}

fn string_list(elements: &[DataValue]) -> Option<Vec<&str>> {
    elements.iter().map(|e| match e {
        DataValue::String(s) => Some(s.as_str()),
        _ => None,
    }).collect()
}

impl Node for MatchNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false }];
        pins.extend(self.inputs.iter().map(|i| PinDefinition {
            id: PinId(i.pin),
            name: i.name.clone(),
            pin_type: PinType::Data(i.value_type.clone()),
            direction: PinDirection::Input, required: false }));
        pins.extend(self.cases.iter().map(|c| PinDefinition {
            id: PinId(c.pin),
            name: c.display(),
            pin_type: PinType::Exec,
            direction: PinDirection::Output, required: false }));
        pins.push(PinDefinition { id: Self::ELSE_PIN, name: "Else".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false });
        NodeDefinition {
            type_name: "match".to_string(),
            title: "Match".to_string(),
            category: "Control Flow".to_string(),
            description: "Обирає вихід за умовами, як комбінатор-вирішувач".to_string(),
            help: "За сигналом на In перевіряє випадки згори вниз і передає виконання на вихід першого, що збігся. Якщо не збігся жоден — на Else. З прапорцем «Усі збіги» спрацьовують усі випадки, що збіглись.\n\nВходи: «+ вхід» додає вхідний пін. Ім'я входу можна змінити (умови оновляться), тип — обрати; «auto» приймає будь-що, і тоді оператори підбираються за типом під'єднаного джерела. Непід'єднаному входу можна задати значення прямо на ноді.\n\nУмова — «вхід  оператор  операнд». Оператори залежать від типу входу:\n• числа, час, тривалість — =, ≠, <, ≤, >, ≥ (для часу — раніше / пізніше);\n• рядки — дорівнює, містить, починається з, закінчується на, порожній, довжина;\n• логічні — = true / = false;\n• списки — містить, порожній, довжина;\n• об'єкти — має поле;\n• будь-який — є значення / немає значення.\nПраворуч — значення або інший вхід (так порівнюються дві змінні). Значення приводиться до типу входу: «07:30» — час, «1h 30m» або 90 — тривалість, «#ff0000» — колір.\n\nУмови з'єднуються «і» / «або»; «і» зв'язує сильніше: «a > 1 і b > 1 або c» = «(a > 1 і b > 1) або c». Випадок без умов спрацьовує завжди. Вхід без значення умову не виконує (крім «немає значення»).".to_string(),
            pins,
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let value_of = |name: &str| {
            self.inputs.iter().find(|i| i.name == name).and_then(|i| ctx.get_value(PinId(i.pin)))
        };
        let mut outputs = Vec::new();
        for case in &self.cases {
            if case.matches(value_of) {
                outputs.push(NodeOutput::Execution(PinId(case.pin)));
                if !self.all_matches {
                    break;
                }
            }
        }
        if outputs.is_empty() {
            outputs.push(NodeOutput::Execution(Self::ELSE_PIN));
        }
        outputs
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        let list = |items: Vec<String>| DataValue::List {
            element_type: DataType::String,
            elements: items.into_iter().map(DataValue::String).collect(),
        };
        // Входи — перед випадками: при відтворенні ноди випадки не мусять виводити входи самі
        vec![
            ("inputs".to_string(), list(self.inputs.iter().map(MatchInput::to_text).collect())),
            ("cases".to_string(), list(self.cases.iter().map(MatchCase::to_text).collect())),
            ("all_matches".to_string(), DataValue::Bool(self.all_matches)),
        ]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        // Некоректний список не застосовується зовсім (редактор перевіряє його заздалегідь)
        match (name, value) {
            ("cases", DataValue::List { elements, .. }) => {
                let parsed: Result<Vec<_>, _> = string_list(&elements)
                    .ok_or_else(|| "випадок має бути рядком".to_string())
                    .and_then(|l| l.into_iter().map(MatchCase::parse).collect());
                match parsed {
                    Ok(cases) => self.set_cases(cases),
                    Err(e) => tracing::warn!("Match: некоректні випадки: {e}"),
                }
            }
            ("inputs", DataValue::List { elements, .. }) => {
                let parsed: Result<Vec<_>, _> = string_list(&elements)
                    .ok_or_else(|| "вхід має бути рядком".to_string())
                    .and_then(|l| l.into_iter().map(MatchInput::parse).collect());
                match parsed {
                    Ok(inputs) => self.set_inputs(inputs),
                    Err(e) => tracing::warn!("Match: некоректні входи: {e}"),
                }
            }
            ("all_matches", DataValue::Bool(b)) => self.all_matches = b,
            _ => {}
        }
    }
    fn restored(&mut self) {
        self.explicit_inputs = true;
    }
}

pub struct ClockNode { pub id: NodeId, pub interval_ticks: u64 }
impl Node for ClockNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "clock".to_string(),
            title: "Clock".to_string(),
            category: "Events".to_string(),
            description: "Генерує імпульс виконання кожні interval_ticks тактів".to_string(),
            help: "Запускає виконання сама: кожні N тактів (властивість «Інтервал») видає сигнал на Out.\n\nТакт сервера — 100 мс, тож 10 тактів ≈ 1 секунда. Це основне джерело подій: з нього починаються ланцюжки, що мають працювати постійно.".to_string(),
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Out".to_string(),
                pin_type: PinType::Exec,
                direction: PinDirection::Output, required: false }],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Execution(PinId(0))]
    }
    fn poll(&mut self, host: &mut HostState) -> Option<PinId> {
        (host.current_tick % self.interval_ticks.max(1) == 0).then_some(PinId(0))
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "interval_ticks" {
            if let DataValue::Number(n) = value {
                self.interval_ticks = n.to_f64().max(1.0) as u64;
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("interval_ticks".to_string(), DataValue::Number(NumberValue::Int(self.interval_ticks as i64)))]
    }
}

pub struct ConsoleNode { pub id: NodeId }
impl Node for ConsoleNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "console".to_string(),
            title: "Console Log".to_string(),
            category: "Debug".to_string(),
            description: "Виводить значення в консоль при отриманні сигналу виконання".to_string(),
            help: "За сигналом на In виводить значення Val у консоль редактора й у лог сервера. Без під'єднаного Val виводить «None».\n\nВиходу немає — це кінцева нода ланцюжка. Щоб вивести значення посеред ланцюжка, використайте Debug Log.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Val".to_string(), pin_type: PinType::Data(DataType::Any), direction: PinDirection::Input, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_value(PinId(1));
        let message = val.as_ref().map(format_data_value).unwrap_or_else(|| "None".to_string());
        info!(%message, "Console Output");
        ctx.host_state.logs.push((self.id, message));
        vec![]
    }
}

pub struct ClickButtonNode {
    pub id: NodeId,
    pub label: String,
    pub clicked: bool,
}

impl Node for ClickButtonNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/click_button".to_string(),
            title: "Click Button".to_string(),
            category: "Events".to_string(),
            description: "Генерує exec сигнал на наступному тіку після того як користувач клікне на кнопку".to_string(),
            help: "Видає сигнал на Out, коли ви натискаєте «Запустити» в правій панелі.\n\nЗручно, щоб вручну запустити ланцюжок під час налагодження.".to_string(),
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "Out".to_string(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Output, required: false }
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        self.clicked = false;
        vec![NodeOutput::Execution(PinId(0))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "label" {
            if let DataValue::String(s) = value {
                self.label = s;
            }
        } else if name == "clicked" {
            if let DataValue::Bool(b) = value {
                self.clicked = b;
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("label".to_string(), DataValue::String(self.label.clone()))]
    }
}

pub fn format_data_value(val: &DataValue) -> String {
    match val {
        DataValue::Bool(b) => b.to_string(),
        DataValue::Number(num) => match num {
            NumberValue::Int(i) => i.to_string(),
            NumberValue::Float(f) => f.to_string(),
        },
        DataValue::String(s) => s.clone(),
        DataValue::Time(t) => t.to_string(),
        DataValue::DateTime(dt) => dt.to_string(),
        DataValue::Duration(d) => format!("{:?}", d),
        DataValue::Color(c) => format!("Color(r:{}, g:{}, b:{}, a:{})", c.r, c.g, c.b, c.a),
        DataValue::List { elements, .. } => {
            let items: Vec<String> = elements.iter().map(format_data_value).collect();
            format!("[{}]", items.join(", "))
        }
        DataValue::Object { fields, .. } => {
            let mut items: Vec<String> = fields.iter()
                .map(|(k, v)| format!("{}: {}", k, format_data_value(&v.value)))
                .collect();
            items.sort();
            format!("{{{}}}", items.join(", "))
        }
    }
}

pub struct DebugNode {
    pub id: NodeId,
    pub prefix: String,
}

impl Node for DebugNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/debug".to_string(),
            title: "Debug Log".to_string(),
            category: "Debug".to_string(),
            description: "Приймає будь-які дані та виводить їх у лог з текстовим описом".to_string(),
            help: "За сигналом на In виводить у консоль «Префікс» (з правої панелі) разом зі значенням Val і передає виконання далі на Out.\n\nЇї можна вставити посеред ланцюжка, щоб подивитися, яке значення через нього проходить.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Val".to_string(), pin_type: PinType::Data(DataType::Any), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Out".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_value(PinId(1));
        let formatted = match val {
            Some(v) => format_data_value(&v),
            None => "None".to_string(),
        };
        let message = format!("{}{}", self.prefix, formatted);
        info!("{}", message);
        ctx.host_state.logs.push((self.id, message));
        vec![NodeOutput::Execution(PinId(2))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "prefix" {
            if let DataValue::String(s) = value {
                self.prefix = s;
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("prefix".to_string(), DataValue::String(self.prefix.clone()))]
    }
}

// Helper to parse fields_list
fn parse_fields_list(s: &str) -> Vec<(String, DataType)> {
    let mut fields = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() { continue; }
        if let Some((name, ty_str)) = part.split_once(':') {
            let name = name.trim().to_string();
            let ty = parse_data_type(ty_str).unwrap_or(DataType::String);
            fields.push((name, ty));
        } else {
            fields.push((part.to_string(), DataType::String));
        }
    }
    fields
}

pub struct CompareNode { pub id: NodeId }
impl Node for CompareNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "compare".to_string(),
            title: "Compare".to_string(),
            category: "Logic".to_string(),
            description: "Порівнює два числа".to_string(),
            help: "Порівнює числа A і B:\n• Eq — A дорівнює B (з точністю до 1e-9);\n• Gt — A більше за B;\n• Lt — A менше за B.\n\nНепід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається нулем.\n\nДля розгалуження виконання за умовою зручніша нода Match.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "B".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Eq".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(3), name: "Gt".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(4), name: "Lt".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a = ctx.get_number(PinId(0), NumberValue::Int(0)).to_f64();
        let b = ctx.get_number(PinId(1), NumberValue::Int(0)).to_f64();
        vec![
            NodeOutput::Data(PinId(2), DataValue::Bool((a - b).abs() < 1e-9)),
            NodeOutput::Data(PinId(3), DataValue::Bool(a > b)),
            NodeOutput::Data(PinId(4), DataValue::Bool(a < b)),
        ]
    }
}

pub struct ForLoopNode {
    pub id: NodeId,
    pub active: bool,
    pub current_index: i64,
    pub count: i64,
}
impl Node for ForLoopNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "for_loop".to_string(),
            title: "For Loop".to_string(),
            category: "Control Flow".to_string(),
            description: "Асинхронний цикл від 0 до Count-1 з кроком 1".to_string(),
            help: "Покроковий цикл:\n1. Сигнал на In починає цикл з Index = 0 і видає сигнал на LoopBody.\n2. Коли тіло циклу відпрацювало, подайте сигнал на Next — Index збільшиться на 1, і LoopBody спрацює знову.\n3. Після Count ітерацій або сигналу на Break спрацьовує Completed.\n\nЯкщо Count ≤ 0, одразу спрацьовує Completed. Кожен крок займає щонайменше такт, тож цикл не блокує рушій.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Count".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Break".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(3), name: "Next".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(4), name: "LoopBody".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(5), name: "Index".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(6), name: "Completed".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if trigger_pin == Some(PinId(0)) {
            let count_val = ctx.get_number(PinId(1), NumberValue::Int(0)).to_f64() as i64;
            if count_val <= 0 {
                self.active = false;
                vec![NodeOutput::Execution(PinId(6))]
            } else {
                self.active = true;
                self.current_index = 0;
                self.count = count_val;
                vec![
                    NodeOutput::Execution(PinId(4)),
                    NodeOutput::Data(PinId(5), DataValue::Number(NumberValue::Int(0))),
                ]
            }
        } else if trigger_pin == Some(PinId(2)) {
            if self.active {
                self.active = false;
                vec![NodeOutput::Execution(PinId(6))]
            } else {
                vec![]
            }
        } else if trigger_pin == Some(PinId(3)) {
            if self.active {
                self.current_index += 1;
                if self.current_index >= self.count {
                    self.active = false;
                    vec![NodeOutput::Execution(PinId(6))]
                } else {
                    vec![
                        NodeOutput::Execution(PinId(4)),
                        NodeOutput::Data(PinId(5), DataValue::Number(NumberValue::Int(self.current_index))),
                    ]
                }
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
}

pub struct SleepNode { pub id: NodeId }
impl Node for SleepNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "sleep".to_string(),
            title: "Sleep".to_string(),
            category: "Time".to_string(),
            description: "Затримує виконання на вказану кількість мілісекунд або тактів".to_string(),
            help: "Сигнал на In запускає затримку, а коли вона мине — видає сигнал на Out.\n\n• Delay — тривалість у мілісекундах (округлюється вгору до цілих тактів).\n• UseTicks = true — Delay рахується в тактах.\n• TickDuration — тривалість такту в мс для розрахунку (типово — такт сервера).\n\nКожен сигнал на In запускає власну затримку, як у Queue Delay.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Delay".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "UseTicks".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(3), name: "TickDuration".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(4), name: "Out".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if trigger_pin == Some(PinId(0)) {
            let delay = ctx.get_number(PinId(1), NumberValue::Int(0)).to_f64().max(0.0);
            let ticks = if ctx.get_bool(PinId(2), false) {
                delay.ceil() as u64
            } else {
                // Мілісекунди; TickDuration (якщо задано) перекриває тривалість такту рушія.
                match ctx.get_number(PinId(3), NumberValue::Int(0)).to_f64() {
                    tick_ms if tick_ms > 0.0 => (delay / tick_ms).ceil() as u64,
                    _ => ctx.host_state.ticks_for(std::time::Duration::from_millis(delay as u64)),
                }
            };
            ctx.host_state.start_delay(self.id, ticks);
        }
        vec![]
    }
    fn on_task(&mut self, _task: TaskId, _result: TaskResult, _ctx: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Execution(PinId(4))]
    }
}

pub struct ListCreateNode {
    pub id: NodeId,
    pub element_type: DataType,
}
impl Node for ListCreateNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "list/create".to_string(),
            title: "Create List".to_string(),
            category: "Lists".to_string(),
            description: "Створює порожній список".to_string(),
            help: "Видає порожній список. Тип елементів обирається в правій панелі.\n\nДалі список наповнюють нодою List Add.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "List".to_string(),
                    pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))),
                    direction: PinDirection::Output, required: false }
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(
            PinId(0),
            DataValue::List {
                element_type: self.element_type.clone(),
                elements: Vec::new(),
            }
        )]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "element_type" {
            if let DataValue::String(s) = value {
                self.element_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListAddNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListAddNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "list/add".to_string(),
            title: "List Add".to_string(),
            category: "Lists".to_string(),
            description: "Додає елемент до списку".to_string(),
            help: "Видає OutList — копію списку List з елементом Item у кінці. Вхідний список не змінюється.\n\nТип елементів обирається в правій панелі й має збігатися з типом списку.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Item".to_string(), pin_type: PinType::Data(self.element_type.clone()), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "OutList".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        let item_val = ctx.get_value(PinId(1));
        if let (Some(DataValue::List { element_type, mut elements }), Some(item)) = (list_val, item_val) {
            elements.push(item);
            vec![NodeOutput::Data(PinId(2), DataValue::List { element_type, elements })]
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "element_type" {
            if let DataValue::String(s) = value {
                self.element_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListRemoveNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListRemoveNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "list/remove".to_string(),
            title: "List Remove".to_string(),
            category: "Lists".to_string(),
            description: "Вилучає елемент зі списку за індексом".to_string(),
            help: "Видає OutList — копію списку без елемента з номером Index (рахунок від 0). Якщо такого номера немає, список повертається без змін.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Index".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "OutList".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        let index_val = ctx.get_number(PinId(1), NumberValue::Int(-1)).to_f64() as i64;
        if let Some(DataValue::List { element_type, mut elements }) = list_val {
            if index_val >= 0 && index_val < elements.len() as i64 {
                elements.remove(index_val as usize);
            }
            vec![NodeOutput::Data(PinId(2), DataValue::List { element_type, elements })]
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "element_type" {
            if let DataValue::String(s) = value {
                self.element_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListGetNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListGetNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "list/get".to_string(),
            title: "List Get".to_string(),
            category: "Lists".to_string(),
            description: "Отримує елемент зі списку за індексом".to_string(),
            help: "Видає Item — елемент списку з номером Index (рахунок від 0). Якщо такого номера немає, вихід не має значення.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Index".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Item".to_string(), pin_type: PinType::Data(self.element_type.clone()), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        let index_val = ctx.get_number(PinId(1), NumberValue::Int(-1)).to_f64() as i64;
        if let Some(DataValue::List { elements, .. }) = list_val {
            if index_val >= 0 && index_val < elements.len() as i64 {
                vec![NodeOutput::Data(PinId(2), elements[index_val as usize].clone())]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "element_type" {
            if let DataValue::String(s) = value {
                self.element_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListLengthNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListLengthNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "list/length".to_string(),
            title: "List Length".to_string(),
            category: "Lists".to_string(),
            description: "Повертає довжину списку".to_string(),
            help: "Видає Len — кількість елементів у списку. Без під'єднаного списку — 0.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Len".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        if let Some(DataValue::List { elements, .. }) = list_val {
            vec![NodeOutput::Data(PinId(1), DataValue::Number(NumberValue::Int(elements.len() as i64)))]
        } else {
            vec![NodeOutput::Data(PinId(1), DataValue::Number(NumberValue::Int(0)))]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "element_type" {
            if let DataValue::String(s) = value {
                self.element_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ObjectCreateNode { pub id: NodeId }
impl Node for ObjectCreateNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "object/create".to_string(),
            title: "Create Object".to_string(),
            category: "Objects".to_string(),
            description: "Створює порожній об'єкт".to_string(),
            help: "Видає порожній об'єкт — набір іменованих полів.\n\nПоля додають нодою Set Object Field.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(PinId(0), DataValue::Object { fields: std::collections::BTreeMap::new(), type_name: None })]
    }
}

pub struct ObjectSetFieldNode { pub id: NodeId, pub field_type: DataType }
impl Node for ObjectSetFieldNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "object/set_field".to_string(),
            title: "Set Object Field".to_string(),
            category: "Objects".to_string(),
            description: "Встановлює значення поля об'єкта".to_string(),
            help: "Видає OutObj — копію об'єкта Obj, у якій поле Name має значення Val. Якщо поля не було, воно додається, інакше — замінюється.\n\nТип Val обирається в правій панелі. Поки не під'єднано всі входи, результату немає.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Name".to_string(), pin_type: PinType::Data(DataType::String), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Val".to_string(), pin_type: PinType::Data(self.field_type.clone()), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(3), name: "OutObj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let obj_val = ctx.get_value(PinId(0));
        let name_val = ctx.get_value(PinId(1));
        let val = ctx.get_value(PinId(2));
        if let (Some(DataValue::Object { mut fields, .. }), Some(DataValue::String(name)), Some(v)) = (obj_val, name_val, val) {
            fields.insert(name.clone(), ObjectField {
                name,
                description: String::new(),
                value: v,
            });
            vec![NodeOutput::Data(PinId(3), DataValue::Object { fields, type_name: None })]
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "field_type" {
            if let DataValue::String(s) = value {
                self.field_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("field_type".to_string(), DataValue::String(data_type_name(&self.field_type)))]
    }
}

pub struct ObjectGetFieldNode { pub id: NodeId, pub field_type: DataType }
impl Node for ObjectGetFieldNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "object/get_field".to_string(),
            title: "Get Object Field".to_string(),
            category: "Objects".to_string(),
            description: "Отримує значення поля об'єкта".to_string(),
            help: "Шукає в об'єкті Obj поле Name:\n• Val — значення поля;\n• Found — чи знайдено поле.\n\nТип Val обирається в правій панелі.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Name".to_string(), pin_type: PinType::Data(DataType::String), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Val".to_string(), pin_type: PinType::Data(self.field_type.clone()), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(3), name: "Found".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let obj_val = ctx.get_value(PinId(0));
        let name_val = ctx.get_value(PinId(1));
        if let (Some(DataValue::Object { fields, .. }), Some(DataValue::String(name))) = (obj_val, name_val) {
            if let Some(field) = fields.get(&name) {
                vec![
                    NodeOutput::Data(PinId(2), field.value.clone()),
                    NodeOutput::Data(PinId(3), DataValue::Bool(true)),
                ]
            } else {
                vec![NodeOutput::Data(PinId(3), DataValue::Bool(false))]
            }
        } else {
            vec![NodeOutput::Data(PinId(3), DataValue::Bool(false))]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "field_type" {
            if let DataValue::String(s) = value {
                self.field_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("field_type".to_string(), DataValue::String(data_type_name(&self.field_type)))]
    }
}

pub struct ObjectUnpackNode {
    pub id: NodeId,
    pub fields_list: String,
    pub parsed_fields: Vec<(String, DataType)>,
}
impl Node for ObjectUnpackNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![
            PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Input, required: false },
        ];
        for (i, (name, ty)) in self.parsed_fields.iter().enumerate() {
            pins.push(PinDefinition {
                id: PinId((i + 1) as u32),
                name: name.clone(),
                pin_type: PinType::Data(ty.clone()),
                direction: PinDirection::Output, required: false });
        }
        NodeDefinition {
            type_name: "object/unpack".to_string(),
            title: "Unpack Object".to_string(),
            category: "Objects".to_string(),
            description: "Розпаковує поля об'єкта".to_string(),
            help: "Видає кілька полів об'єкта одразу. Поля перелічують на ноді або в правій панелі через кому у вигляді «назва:тип», наприклад `temp:number, city:string` (без типу — string).\n\nКожне поле стає окремим вихідним піном. Поле, якого немає в об'єкті, не має значення.\n\nЧиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.".to_string(),
            pins,
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let obj_val = ctx.get_value(PinId(0));
        let mut outputs = Vec::new();
        if let Some(DataValue::Object { fields, .. }) = obj_val {
            for (i, (name, _)) in self.parsed_fields.iter().enumerate() {
                if let Some(field) = fields.get(name) {
                    outputs.push(NodeOutput::Data(PinId((i + 1) as u32), field.value.clone()));
                }
            }
        }
        outputs
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "fields_list" {
            if let DataValue::String(s) = value {
                self.fields_list = s;
                self.parsed_fields = parse_fields_list(&self.fields_list);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("fields_list".to_string(), DataValue::String(self.fields_list.clone()))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Нода-приймач з exec-входом PinId(0), щоб відстежувати потік виконання.
    fn add_sink(engine: &mut TickEngine) -> NodeId {
        let id = NodeId::generate();
        engine.create_node(id, "test/debug", &[]).unwrap();
        id
    }

    fn cases(list: &[&str]) -> (String, DataValue) {
        ("cases".to_string(), DataValue::List {
            element_type: DataType::String,
            elements: list.iter().map(|c| DataValue::String(c.to_string())).collect(),
        })
    }

    fn num(v: i64) -> DataValue {
        DataValue::Number(NumberValue::Int(v))
    }

    /// Запускає ноду й повертає exec-піни, що спрацювали.
    fn fire(engine: &mut TickEngine, id: NodeId) -> Vec<PinId> {
        engine.trigger_node(id, PinId(0));
        engine.tick();
        engine.executed.iter().find(|t| t.node_id == id).map(|t| t.fired.clone()).unwrap_or_default()
    }

    #[test]
    fn test_match_cases_and_else() {
        let mut engine = TickEngine::new();
        let id = NodeId::generate();
        engine.create_node(id, "match", &[cases(&["1: t > 25", "2: t > 20 and mode = \"auto\"", "3: t < 0 or t > 100"])]).unwrap();
        let def = engine.node_definition(id).unwrap();
        let inputs: Vec<_> = def.pins.iter().filter(|p| p.direction == PinDirection::Input).map(|p| p.name.as_str()).collect();
        assert_eq!(inputs, ["In", "mode", "t"]);
        let outputs: Vec<_> = def.pins.iter().filter(|p| p.direction == PinDirection::Output).map(|p| p.name.as_str()).collect();
        assert_eq!(outputs, ["t > 25", "t > 20 і mode = \"auto\"", "t < 0 або t > 100", "Else"]);

        // Входи без значень — жодна умова не виконується
        assert_eq!(fire(&mut engine, id), [MatchNode::ELSE_PIN]);

        engine.set_input_value(id, PinId(2), Some(num(30))).unwrap();
        assert_eq!(fire(&mut engine, id), [PinId(1)], "лише перший збіг");

        engine.set_node_property(id, "all_matches", DataValue::Bool(true)).unwrap();
        engine.set_input_value(id, PinId(1), Some(DataValue::String("auto".into()))).unwrap();
        engine.set_input_value(id, PinId(2), Some(num(120))).unwrap();
        assert_eq!(fire(&mut engine, id), [PinId(1), PinId(2), PinId(3)]);

        engine.set_input_value(id, PinId(2), Some(num(10))).unwrap();
        assert_eq!(fire(&mut engine, id), [MatchNode::ELSE_PIN]);
    }

    #[test]
    fn test_match_rejects_invalid_cases() {
        let mut engine = TickEngine::new();
        let id = NodeId::generate();
        engine.create_node(id, "match", &[cases(&["5: a > 1", "5: a < 0", "0: a = 3"])]).unwrap();
        // Повтори номерів і пін Else відкидаються
        assert_eq!(engine.node_model(id).unwrap().properties[1], cases(&["5: a > 1"]));
        // Некоректний список не застосовується
        engine.set_node_property(id, "cases", cases(&["1: a >"]).1).unwrap();
        assert_eq!(engine.node_model(id).unwrap().properties[1], cases(&["5: a > 1"]));
    }

    fn inputs(list: &[&str]) -> (String, DataValue) {
        ("inputs".to_string(), cases(list).1)
    }

    #[test]
    fn test_match_explicit_inputs_and_types() {
        let mut engine = TickEngine::new();
        let id = NodeId::generate();
        engine.create_node(id, "match", &[
            inputs(&["1:now:time", "2:limit:time", "3:list:list", "3:dup:any", "4:list:any"]),
            cases(&["1: now > limit", "2: list contains 3", "3: now >= \"22:00\""]),
        ]).unwrap();
        let def = engine.node_definition(id).unwrap();
        let ins: Vec<_> = def.pins.iter().filter(|p| p.direction == PinDirection::Input).map(|p| (p.id.0, p.name.as_str())).collect();
        // Повтори пінів і імен відкинуто
        assert_eq!(ins, [(0, "In"), (1, "now"), (2, "limit"), (3, "list")]);
        assert!(def.pins.iter().any(|p| p.id == PinId(1) && p.pin_type == PinType::Data(DataType::Time)));

        let time = |t: &str| DataValue::Time(ticknode_shared::text::parse_time(t).unwrap());
        engine.set_input_value(id, PinId(1), Some(time("22:30"))).unwrap();
        engine.set_input_value(id, PinId(2), Some(time("23:00"))).unwrap();
        // now < limit, список порожній, але now ≥ 22:00 (рядок приводиться до часу)
        assert_eq!(fire(&mut engine, id), [PinId(3)]);
        engine.set_input_value(id, PinId(2), Some(time("21:00"))).unwrap();
        assert_eq!(fire(&mut engine, id), [PinId(1)]);

        // Явні входи переживають збереження
        let graph = engine.export_graph();
        let mut copy = TickEngine::new();
        assert!(copy.load_graph(&graph).is_empty());
        assert_eq!(copy.node_definition(id).unwrap().pins, def.pins);
    }

    #[test]
    fn test_match_inputs_are_explicit_after_creation() {
        // Нова нода й та сама нода після перезапуску однаково реагують на зміну випадків
        let mut engine = TickEngine::new();
        let id = NodeId::generate();
        engine.create_node(id, "match", &[]).unwrap();
        engine.set_node_property(id, "cases", cases(&["1: x > 0"]).1).unwrap();
        let fresh = engine.node_definition(id).unwrap().pins;

        let mut copy = TickEngine::new();
        copy.create_node(id, "match", &[]).unwrap();
        let graph = copy.export_graph();
        let mut copy = TickEngine::new();
        assert!(copy.load_graph(&graph).is_empty());
        copy.set_node_property(id, "cases", cases(&["1: x > 0"]).1).unwrap();
        assert_eq!(copy.node_definition(id).unwrap().pins, fresh);
        // Вхід `a` лишився, `x` не з'явився сам
        assert!(fresh.iter().any(|p| p.name == "a") && !fresh.iter().any(|p| p.name == "x"));
    }

    #[test]
    fn test_input_values() {
        let mut engine = TickEngine::new();
        let add = NodeId::generate();
        engine.create_node(add, "math/add", &[]).unwrap();
        engine.set_input_value(add, PinId(0), Some(num(2))).unwrap();
        engine.set_input_value(add, PinId(1), Some(num(3))).unwrap();
        assert_eq!(engine.pull_data(add, PinId(2), &mut Vec::new()), Some(num(5)));

        // Тип має збігатися з піном, а пін — бути входом даних
        assert!(engine.set_input_value(add, PinId(0), Some(DataValue::String("x".into()))).is_err());
        assert!(engine.set_input_value(add, PinId(2), Some(num(1))).is_err());

        // Під'єднане джерело важливіше за значення на ноді
        let c = NodeId::generate();
        engine.create_node(c, "const", &[("value".into(), num(10))]).unwrap();
        engine.connect_data(c, PinId(0), add, PinId(0)).unwrap();
        assert_eq!(engine.pull_data(add, PinId(2), &mut Vec::new()), Some(num(13)));
        engine.disconnect_data(add, PinId(0));
        assert_eq!(engine.pull_data(add, PinId(2), &mut Vec::new()), Some(num(5)));

        // Значення зберігаються як властивості й відновлюються
        let graph = engine.export_graph();
        let mut copy = TickEngine::new();
        assert!(copy.load_graph(&graph).is_empty());
        assert_eq!(copy.pull_data(add, PinId(2), &mut Vec::new()), Some(num(5)));

        // Пін зник після зміни властивості — його значення прибирається
        let m = NodeId::generate();
        engine.create_node(m, "match", &[cases(&["1: a > b"])]).unwrap();
        engine.set_input_value(m, PinId(2), Some(num(1))).unwrap();
        engine.set_node_property(m, "inputs", inputs(&["1:a:any"]).1).unwrap();
        assert!(engine.node_model(m).unwrap().properties.iter().all(|(n, _)| parse_input_value_property(n).is_none()));
        engine.set_input_value(add, PinId(1), None).unwrap();
        assert_eq!(engine.node_model(add).unwrap().properties, vec![(input_value_property(PinId(0)), num(2))]);
    }

    #[test]
    fn test_every_built_in_node_has_help() {
        let engine = TickEngine::new();
        for def in engine.registry.get_definitions() {
            assert!(!def.help.trim().is_empty(), "нода {} без пояснення", def.type_name);
        }
    }

    #[test]
    fn test_built_in_logic_nodes() {
        let mut engine = TickEngine::new();

        // Match з одним випадком «a = true» на пін 2 — колишній Branch
        let branch_id = NodeId::generate();
        engine.create_node(branch_id, "match", &[cases(&["2: a = true"])]).unwrap();

        // Compare
        let compare_id = NodeId::generate();
        let compare = engine.registry.create_node("compare", compare_id).unwrap();
        engine.add_node(compare);

        // Connect compare inputs to Const nodes
        let const_a_id = NodeId::generate();
        let const_a = Box::new(ConstNode { id: const_a_id, value: DataValue::Number(NumberValue::Int(10)) });
        engine.add_node(const_a);

        let const_b_id = NodeId::generate();
        let const_b = Box::new(ConstNode { id: const_b_id, value: DataValue::Number(NumberValue::Int(5)) });
        engine.add_node(const_b);

        engine.connect_data(const_a_id, PinId(0), compare_id, PinId(0)).unwrap();
        engine.connect_data(const_b_id, PinId(0), compare_id, PinId(1)).unwrap();

        // Pull compare outputs to verify
        let mut stack = Vec::new();
        let eq_val = engine.pull_data(compare_id, PinId(2), &mut stack).unwrap();
        let gt_val = engine.pull_data(compare_id, PinId(3), &mut stack).unwrap();
        let lt_val = engine.pull_data(compare_id, PinId(4), &mut stack).unwrap();

        assert_eq!(eq_val, DataValue::Bool(false));
        assert_eq!(gt_val, DataValue::Bool(true));
        assert_eq!(lt_val, DataValue::Bool(false));

        // Compare Gt → вхід a ноди Match
        engine.connect_data(compare_id, PinId(3), branch_id, PinId(1)).unwrap();

        // Запуск Match
        engine.trigger_node(branch_id, PinId(0));
        
        // Випадок 2 → приймач
        let target_id = add_sink(&mut engine);
        engine.connect_exec(branch_id, PinId(2), target_id, PinId(0)).unwrap();

        engine.tick();
        // Gt = true, тож спрацьовує пін 2 і приймач стає в чергу
        assert!(engine.execution_queue.contains(&(target_id, PinId(0))));
    }

    #[test]
    fn test_built_in_for_loop_and_sleep() {
        let mut engine = TickEngine::new();

        // ForLoop
        let loop_id = NodeId::generate();
        let for_loop = engine.registry.create_node("for_loop", loop_id).unwrap();
        engine.add_node(for_loop);

        // Connect Loop Count to Const
        let const_count_id = NodeId::generate();
        let const_count = Box::new(ConstNode { id: const_count_id, value: DataValue::Number(NumberValue::Int(3)) });
        engine.add_node(const_count);
        engine.connect_data(const_count_id, PinId(0), loop_id, PinId(1)).unwrap();

        // Connect LoopBody and Completed to dummy target nodes to trace exec flow
        let dummy_body_id = add_sink(&mut engine);
        let dummy_completed_id = add_sink(&mut engine);
        engine.connect_exec(loop_id, PinId(4), dummy_body_id, PinId(0)).unwrap();
        engine.connect_exec(loop_id, PinId(6), dummy_completed_id, PinId(0)).unwrap();

        // Trigger In (Pin 0)
        engine.trigger_node(loop_id, PinId(0));
        engine.tick();

        // It should output LoopBody (Pin 4) -> should trigger dummy_body
        assert_eq!(engine.data_cache.get(&loop_id).unwrap().get(&PinId(5)), Some(&DataValue::Number(NumberValue::Int(0))));
        assert!(engine.execution_queue.contains(&(dummy_body_id, PinId(0))));
        engine.execution_queue.clear();

        // Trigger Next (Pin 3)
        engine.trigger_node(loop_id, PinId(3));
        engine.tick();
        // Index = 1, LoopBody
        assert_eq!(engine.data_cache.get(&loop_id).unwrap().get(&PinId(5)), Some(&DataValue::Number(NumberValue::Int(1))));
        assert!(engine.execution_queue.contains(&(dummy_body_id, PinId(0))));
        engine.execution_queue.clear();

        // Trigger Next (Pin 3)
        engine.trigger_node(loop_id, PinId(3));
        engine.tick();
        // Index = 2, LoopBody
        assert_eq!(engine.data_cache.get(&loop_id).unwrap().get(&PinId(5)), Some(&DataValue::Number(NumberValue::Int(2))));
        assert!(engine.execution_queue.contains(&(dummy_body_id, PinId(0))));
        engine.execution_queue.clear();

        // Trigger Next (Pin 3)
        engine.trigger_node(loop_id, PinId(3));
        engine.tick();
        // Completed (Pin 6) -> should trigger dummy_completed
        assert!(engine.execution_queue.contains(&(dummy_completed_id, PinId(0))));
        engine.execution_queue.clear();

        // --- Sleep Node ---
        let sleep_id = NodeId::generate();
        let sleep_node = engine.registry.create_node("sleep", sleep_id).unwrap();
        engine.add_node(sleep_node);

        // Connect Sleep inputs
        let const_delay_id = NodeId::generate();
        let const_delay = Box::new(ConstNode { id: const_delay_id, value: DataValue::Number(NumberValue::Int(3)) });
        engine.add_node(const_delay);
        engine.connect_data(const_delay_id, PinId(0), sleep_id, PinId(1)).unwrap();

        let const_ticks_id = NodeId::generate();
        let const_ticks = Box::new(ConstNode { id: const_ticks_id, value: DataValue::Bool(true) });
        engine.add_node(const_ticks);
        engine.connect_data(const_ticks_id, PinId(0), sleep_id, PinId(2)).unwrap();

        // Connect Sleep Out to dummy target node to trace exec flow
        let dummy_sleep_out_id = add_sink(&mut engine);
        engine.connect_exec(sleep_id, PinId(4), dummy_sleep_out_id, PinId(0)).unwrap();

        engine.trigger_node(sleep_id, PinId(0)); // trigger In
        engine.tick(); // task created

        assert_eq!(engine.host_state.active_tasks.len(), 1);
        let task_handle = *engine.host_state.active_tasks.keys().next().unwrap();

        engine.tick(); // elapsed = 1
        assert!(engine.host_state.active_tasks.contains_key(&task_handle));

        engine.tick(); // elapsed = 2
        assert!(engine.host_state.active_tasks.contains_key(&task_handle));

        engine.tick(); // elapsed = 3 -> задача завершилась і доставлена ноді, Out ставить наступну ноду в чергу
        assert!(!engine.host_state.active_tasks.contains_key(&task_handle));
        assert!(engine.execution_queue.contains(&(dummy_sleep_out_id, PinId(0))));
    }

    #[test]
    fn test_built_in_collections() {
        let mut engine = TickEngine::new();

        // 1. List
        let create_id = NodeId::generate();
        let mut create_node = engine.registry.create_node("list/create", create_id).unwrap();
        create_node.set_property("element_type", DataValue::String("number".to_string()));
        engine.add_node(create_node);

        let add_id = NodeId::generate();
        let mut add_node = engine.registry.create_node("list/add", add_id).unwrap();
        add_node.set_property("element_type", DataValue::String("number".to_string()));
        engine.add_node(add_node);

        // Connect List from create to add
        engine.connect_data(create_id, PinId(0), add_id, PinId(0)).unwrap();

        // Connect Item to Const
        let const_item_id = NodeId::generate();
        let const_item = Box::new(ConstNode { id: const_item_id, value: DataValue::Number(NumberValue::Int(42)) });
        engine.add_node(const_item);
        engine.connect_data(const_item_id, PinId(0), add_id, PinId(1)).unwrap();

        // Pull output of add_node
        let mut stack = Vec::new();
        let res_list = engine.pull_data(add_id, PinId(2), &mut stack).unwrap();
        if let DataValue::List { element_type, elements } = res_list {
            assert_eq!(element_type, DataType::Number);
            assert_eq!(elements, vec![DataValue::Number(NumberValue::Int(42))]);
        } else {
            panic!("Expected list output");
        }

        // 2. Object
        let obj_create_id = NodeId::generate();
        let obj_create = engine.registry.create_node("object/create", obj_create_id).unwrap();
        engine.add_node(obj_create);

        let obj_set_id = NodeId::generate();
        let mut obj_set = engine.registry.create_node("object/set_field", obj_set_id).unwrap();
        obj_set.set_property("field_type", DataValue::String("number".to_string()));
        engine.add_node(obj_set);

        // Connect obj_create to obj_set
        engine.connect_data(obj_create_id, PinId(0), obj_set_id, PinId(0)).unwrap();

        // Connect Name and Val to Const nodes
        let const_name_id = NodeId::generate();
        let const_name = Box::new(ConstNode { id: const_name_id, value: DataValue::String("age".to_string()) });
        engine.add_node(const_name);
        engine.connect_data(const_name_id, PinId(0), obj_set_id, PinId(1)).unwrap();

        let const_val_id = NodeId::generate();
        let const_val = Box::new(ConstNode { id: const_val_id, value: DataValue::Number(NumberValue::Int(30)) });
        engine.add_node(const_val);
        engine.connect_data(const_val_id, PinId(0), obj_set_id, PinId(2)).unwrap();

        // Pull object
        stack.clear();
        let res_obj = engine.pull_data(obj_set_id, PinId(3), &mut stack).unwrap();

        // Now Unpack
        let unpack_id = NodeId::generate();
        let mut unpack = engine.registry.create_node("object/unpack", unpack_id).unwrap();
        unpack.set_property("fields_list", DataValue::String("age:number,name:string".to_string()));
        engine.add_node(unpack);

        // Set unpack Obj input by creating a Const node wrapper for res_obj
        let const_obj_id = NodeId::generate();
        let const_obj = Box::new(ConstNode { id: const_obj_id, value: res_obj });
        engine.add_node(const_obj);
        engine.connect_data(const_obj_id, PinId(0), unpack_id, PinId(0)).unwrap();

        // Pull age (pin 1) and name (pin 2)
        stack.clear();
        let age_val = engine.pull_data(unpack_id, PinId(1), &mut stack).unwrap();
        let name_val = engine.pull_data(unpack_id, PinId(2), &mut stack); // name is missing, should be None

        assert_eq!(age_val, DataValue::Number(NumberValue::Int(30)));
        assert!(name_val.is_none());
    }

    #[test]
    fn test_click_button_and_debug_nodes() {
        let mut engine = TickEngine::new();

        // 1. Створюємо ClickButton
        let button_id = NodeId::generate();
        let button_node = engine.registry.create_node("test/click_button", button_id).unwrap();
        engine.add_node(button_node);

        // 2. Створюємо DebugNode та задаємо префікс
        let debug_id = NodeId::generate();
        let mut debug_node = engine.registry.create_node("test/debug", debug_id).unwrap();
        debug_node.set_property("prefix", DataValue::String("значення змінної: ".to_string()));
        engine.add_node(debug_node);

        // 3. З'єднуємо Exec: Button Out -> Debug In
        engine.connect_exec(button_id, PinId(0), debug_id, PinId(0)).unwrap();

        // 4. Створюємо ConstNode (444) і з'єднуємо з Debug Val (Any)
        let const_id = NodeId::generate();
        let const_node = Box::new(ConstNode { id: const_id, value: DataValue::Number(NumberValue::Int(444)) });
        engine.add_node(const_node);
        engine.connect_data(const_id, PinId(0), debug_id, PinId(1)).unwrap(); // Any connection must succeed!

        // Симулюємо клік користувача
        engine.set_node_property(button_id, "clicked", DataValue::Bool(true)).unwrap();
        engine.trigger_node(button_id, PinId(0));

        // Перший такт: Кнопка виконується, дебаг нода стає в чергу на наступний такт
        engine.tick();
        assert!(engine.execution_queue.contains(&(debug_id, PinId(0))));

        // Другий такт: Дебаг нода виконується
        engine.tick();
        assert!(engine.execution_queue.is_empty());
    }

    #[test]
    fn test_graph_roundtrip_keeps_properties_and_connections() {
        let mut engine = TickEngine::new();
        let a = NodeId::generate();
        let b = NodeId::generate();
        let dbg = NodeId::generate();
        engine.create_node(a, "const", &[("value".into(), DataValue::String("hi".into()))]).unwrap();
        engine.create_node(b, "math/expression", &[("expression".into(), DataValue::String("x * 2".into()))]).unwrap();
        engine.create_node(dbg, "test/debug", &[("prefix".into(), DataValue::String("> ".into()))]).unwrap();
        engine.connect_data(a, PinId(0), dbg, PinId(1)).unwrap();

        let graph = engine.export_graph();
        let mut restored = TickEngine::new();
        let warnings = restored.load_graph(&graph);
        assert!(warnings.is_empty(), "{:?}", warnings);
        assert_eq!(restored.export_graph(), graph);
        // Пін формули відновився з виразу
        let def = restored.node_definition(b).unwrap();
        assert!(def.pins.iter().any(|p| p.name == "x" && p.direction == PinDirection::Input));
    }

    #[test]
    fn test_data_input_has_single_source_and_exec_is_validated() {
        let mut engine = TickEngine::new();
        let c1 = NodeId::generate();
        let c2 = NodeId::generate();
        let dbg = add_sink(&mut engine);
        engine.create_node(c1, "const", &[]).unwrap();
        engine.create_node(c2, "const", &[]).unwrap();
        engine.connect_data(c1, PinId(0), dbg, PinId(1)).unwrap();
        engine.connect_data(c2, PinId(0), dbg, PinId(1)).unwrap();
        assert_eq!(engine.data_connections().len(), 1);
        assert_eq!(engine.data_connections()[0].src_node, c2);

        // Data-пін не можна під'єднати як exec
        assert_eq!(engine.connect_exec(c1, PinId(0), dbg, PinId(0)), Err(EngineError::InvalidPinConnection));
    }

    #[test]
    fn test_property_change_prunes_invalid_connections() {
        let mut engine = TickEngine::new();
        let x = NodeId::generate();
        let expr = NodeId::generate();
        engine.create_node(x, "const", &[]).unwrap();
        engine.create_node(expr, "math/expression", &[("expression".into(), DataValue::String("a + b".into()))]).unwrap();
        engine.connect_data(x, PinId(0), expr, PinId(2)).unwrap(); // b

        // Тепер у формулі одна змінна — піна 2 більше немає
        let removed = engine.set_node_property(expr, "expression", DataValue::String("a * 3".into())).unwrap();
        assert_eq!(removed.data.len(), 1);
        assert!(engine.data_connections().is_empty());
    }

    #[test]
    fn test_remove_node_cancels_its_tasks() {
        let mut engine = TickEngine::new();
        let sleep = NodeId::generate();
        engine.create_node(sleep, "sleep", &[]).unwrap();
        let delay = NodeId::generate();
        engine.create_node(delay, "const", &[("value".into(), DataValue::Number(NumberValue::Int(100)))]).unwrap();
        engine.connect_data(delay, PinId(0), sleep, PinId(1)).unwrap();
        engine.trigger_node(sleep, PinId(0));
        engine.tick();
        assert_eq!(engine.host_state.active_tasks.len(), 1);

        engine.remove_node(sleep);
        assert!(engine.host_state.active_tasks.is_empty());
    }

    #[test]
    fn test_clock_fires_every_interval() {
        let mut engine = TickEngine::new();
        let clock = NodeId::generate();
        engine.create_node(clock, "clock", &[("interval_ticks".into(), DataValue::Number(NumberValue::Int(3)))]).unwrap();
        let sink = add_sink(&mut engine);
        engine.connect_exec(clock, PinId(0), sink, PinId(0)).unwrap();

        let mut fired_on = Vec::new();
        for _ in 0..9 {
            engine.tick();
            if engine.executed.iter().any(|t| t.node_id == clock) {
                fired_on.push(engine.host_state.current_tick);
            }
        }
        assert_eq!(fired_on, vec![3, 6, 9]);
    }

    #[test]
    fn test_traces_record_inputs_outputs_and_fired_pins() {
        let mut engine = TickEngine::new();
        let num = |v| DataValue::Number(NumberValue::Int(v));
        let (a, b, add, debug) = (NodeId::generate(), NodeId::generate(), NodeId::generate(), NodeId::generate());
        engine.create_node(a, "const", &[("value".into(), num(2))]).unwrap();
        engine.create_node(b, "const", &[("value".into(), num(3))]).unwrap();
        engine.create_node(add, "math/add", &[]).unwrap();
        engine.create_node(debug, "test/debug", &[]).unwrap();
        engine.connect_data(a, PinId(0), add, PinId(0)).unwrap();
        engine.connect_data(b, PinId(0), add, PinId(1)).unwrap();
        engine.connect_data(add, PinId(2), debug, PinId(1)).unwrap();

        engine.trigger_node(debug, PinId(0));
        engine.tick();

        let trace = |id| engine.executed.iter().find(|t| t.node_id == id).cloned().unwrap();
        let d = trace(debug);
        assert_eq!(d.tick, 1);
        assert_eq!(d.inputs, vec![(PinId(1), Some(num(5)))]);
        assert_eq!(d.fired, vec![PinId(2)]);
        // Чиста нода, обчислена на запит, теж має трасу.
        let s = trace(add);
        assert_eq!(s.inputs, vec![(PinId(0), Some(num(2))), (PinId(1), Some(num(3)))]);
        assert_eq!(s.outputs, vec![(PinId(2), num(5))]);

        engine.tick();
        assert!(engine.executed.is_empty());
    }

    #[test]
    fn test_import_snippet_maps_ids_and_keeps_inner_connections() {
        let mut engine = TickEngine::new();
        let (a, b, outside) = (NodeId::generate(), NodeId::generate(), NodeId::generate());
        engine.create_node(a, "const", &[]).unwrap();
        engine.create_node(b, "test/debug", &[]).unwrap();
        engine.create_node(outside, "test/debug", &[]).unwrap();
        engine.connect_data(a, PinId(0), b, PinId(1)).unwrap();
        engine.connect_data(a, PinId(0), outside, PinId(1)).unwrap();
        let mut snippet = engine.export_snippet(&[a, b]);
        // З'єднання на ноду поза фрагментом (напр. з буфера іншого графа) ігнорується.
        snippet.data_connections.push(ConnectionModel { src_node: a, src_pin: PinId(0), dst_node: outside, dst_pin: PinId(1) });

        let (created, warnings) = engine.import_snippet(&snippet);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(created.iter().map(|(old, _)| *old).collect::<Vec<_>>(), vec![a, b]);
        let (new_a, new_b) = (created[0].1, created[1].1);
        assert!(new_a != a && new_b != b);
        assert_eq!(engine.export_snippet(&[new_a, new_b]).data_connections.len(), 1);
        assert_eq!(engine.node_ids().len(), 5);
    }
}