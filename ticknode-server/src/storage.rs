use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use ticknode_core::TickEngine;
use ticknode_shared::{GraphModel, NodeId, Position};
use tracing::{info, warn};
use crate::actor::routes::{self, Routes};

/// The graph is stored as pretty-printed JSON with deterministic ordering so it diffs cleanly in git.
pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Loads the graph into the engine and returns node positions and wire routes. A missing file starts with an empty graph;
    /// a corrupted file returns an error rather than overwriting it with an empty graph.
    pub fn load(&self, engine: &mut TickEngine) -> anyhow::Result<(HashMap<NodeId, Position>, Routes)> {
        if !self.path.exists() {
            info!(path = %self.path.display(), "Graph file not found, starting with an empty graph");
            return Ok((HashMap::new(), Routes::new()));
        }
        let text = std::fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read {}", self.path.display()))?;
        let graph: GraphModel = serde_json::from_str(&text)
            .with_context(|| format!("invalid graph JSON in {}", self.path.display()))?;

        for warning in engine.load_graph(&graph) {
            warn!("{}", warning);
        }
        info!(nodes = graph.nodes.len(), path = %self.path.display(), "Graph loaded");
        Ok((graph.nodes.iter().map(|n| (n.id, n.position)).collect(), routes::from_routes(engine, &graph.wire_routes)))
    }

    pub fn save(&self, engine: &TickEngine, positions: &HashMap<NodeId, Position>, routes: &Routes) -> anyhow::Result<()> {
        let mut graph = engine.export_graph();
        for node in &mut graph.nodes {
            node.position = positions.get(&node.id).copied().unwrap_or_default();
        }
        graph.wire_routes = routes::sorted(routes);
        write_atomic(&self.path, &(serde_json::to_string_pretty(&graph)? + "\n"))
    }
}

/// Writes via a temporary file and rename so a crash mid-write does not corrupt the graph.
fn write_atomic(path: &Path, contents: &str) -> anyhow::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, contents).with_context(|| format!("failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ticknode_shared::{DataValue, PinId};

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::new(dir.path().join("graph.json"));

        let mut engine = TickEngine::new();
        let c = NodeId::generate();
        let dbg = NodeId::generate();
        engine.create_node(c, "const", &[("value".into(), DataValue::String("x".into()))]).unwrap();
        engine.create_node(dbg, "test/debug", &[]).unwrap();
        engine.connect_data(c, PinId(0), dbg, PinId(1)).unwrap();
        let positions = HashMap::from([(c, Position { x: 10.0, y: 20.0 })]);
        let connection = engine.data_connections()[0];
        let routes = Routes::from([((false, connection), vec![Position { x: 48.0, y: -80.0 }])]);
        storage.save(&engine, &positions, &routes).unwrap();

        let mut restored = TickEngine::new();
        let (restored_positions, restored_routes) = storage.load(&mut restored).unwrap();
        assert_eq!(restored_positions[&c], Position { x: 10.0, y: 20.0 });
        assert_eq!(restored.export_graph(), engine.export_graph());
        assert_eq!(restored_routes, routes);

        // Saving again without changes produces an identical file (stable diff)
        let first = std::fs::read_to_string(dir.path().join("graph.json")).unwrap();
        storage.save(&restored, &restored_positions, &restored_routes).unwrap();
        let second = std::fs::read_to_string(dir.path().join("graph.json")).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn broken_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(Storage::new(path).load(&mut TickEngine::new()).is_err());
    }

    #[test]
    fn old_graphs_load_without_routes_and_stale_routes_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph.json");
        let storage = Storage::new(path.clone());
        let mut engine = TickEngine::new();
        assert!(storage.load(&mut engine).unwrap().1.is_empty());
        std::fs::write(&path, r#"{"nodes":[],"exec_connections":[],"data_connections":[]}"#).unwrap();
        assert!(storage.load(&mut engine).unwrap().1.is_empty());
        let route = ticknode_shared::WireRoute {
            exec: true,
            connection: ticknode_shared::ConnectionModel { src_node: NodeId::generate(), src_pin: PinId(0), dst_node: NodeId::generate(), dst_pin: PinId(0) },
            points: vec![Position { x: 4.0, y: 8.0 }],
        };
        let graph = GraphModel { wire_routes: vec![route], ..Default::default() };
        std::fs::write(&path, serde_json::to_string(&graph).unwrap()).unwrap();
        assert!(storage.load(&mut engine).unwrap().1.is_empty());
    }

    #[test]
    fn routes_are_saved_in_a_deterministic_connection_order() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::new(dir.path().join("graph.json"));
        let mut engine = TickEngine::new();
        let a = NodeId::generate();
        let b = NodeId::generate();
        for id in [a, b] { engine.create_node(id, "for_loop", &[]).unwrap(); }
        engine.connect_exec(a, PinId(4), b, PinId(0)).unwrap();
        engine.connect_exec(b, PinId(4), a, PinId(0)).unwrap();
        engine.connect_data(a, PinId(5), b, PinId(1)).unwrap();
        let routes: Routes = routes::connections(&engine).into_iter().map(|key| (key, vec![Position::default()])).collect();
        storage.save(&engine, &HashMap::new(), &routes).unwrap();
        let first = std::fs::read_to_string(dir.path().join("graph.json")).unwrap();
        let graph: GraphModel = serde_json::from_str(&first).unwrap();
        assert!(!graph.wire_routes[0].exec);
        assert!(graph.wire_routes[1].connection.src_node < graph.wire_routes[2].connection.src_node);
        let reordered: Routes = routes.iter().map(|(key, points)| (*key, points.clone())).collect();
        storage.save(&engine, &HashMap::new(), &reordered).unwrap();
        assert_eq!(first, std::fs::read_to_string(dir.path().join("graph.json")).unwrap());
    }
}
