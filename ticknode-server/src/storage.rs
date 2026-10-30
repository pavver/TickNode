use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use ticknode_core::TickEngine;
use ticknode_shared::{GraphModel, NodeId, Position};
use tracing::{info, warn};

/// Граф зберігається як відформатований JSON зі стабільним порядком — щоб нормально лягав у git.
pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Завантажує граф у рушій і повертає позиції нод. Відсутній файл — порожній граф;
    /// пошкоджений файл — помилка, щоб не перезаписати його порожнім графом.
    pub fn load(&self, engine: &mut TickEngine) -> anyhow::Result<HashMap<NodeId, Position>> {
        if !self.path.exists() {
            info!(path = %self.path.display(), "Файл графа не знайдено, починаємо з порожнього");
            return Ok(HashMap::new());
        }
        let text = std::fs::read_to_string(&self.path)
            .with_context(|| format!("не вдалось прочитати {}", self.path.display()))?;
        let graph: GraphModel = serde_json::from_str(&text)
            .with_context(|| format!("некоректний JSON графа у {}", self.path.display()))?;

        for warning in engine.load_graph(&graph) {
            warn!("{}", warning);
        }
        info!(nodes = graph.nodes.len(), path = %self.path.display(), "Граф завантажено");
        Ok(graph.nodes.iter().map(|n| (n.id, n.position)).collect())
    }

    pub fn save(&self, engine: &TickEngine, positions: &HashMap<NodeId, Position>) -> anyhow::Result<()> {
        let mut graph = engine.export_graph();
        for node in &mut graph.nodes {
            node.position = positions.get(&node.id).copied().unwrap_or_default();
        }
        write_atomic(&self.path, &(serde_json::to_string_pretty(&graph)? + "\n"))
    }
}

/// Запис через тимчасовий файл і rename, щоб падіння посеред запису не зіпсувало граф.
fn write_atomic(path: &Path, contents: &str) -> anyhow::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, contents).with_context(|| format!("не вдалось записати {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("не вдалось замінити {}", path.display()))?;
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
        storage.save(&engine, &positions).unwrap();

        let mut restored = TickEngine::new();
        let restored_positions = storage.load(&mut restored).unwrap();
        assert_eq!(restored_positions[&c], Position { x: 10.0, y: 20.0 });
        assert_eq!(restored.export_graph(), engine.export_graph());

        // Повторне збереження без змін дає ідентичний файл (стабільний diff)
        let first = std::fs::read_to_string(dir.path().join("graph.json")).unwrap();
        storage.save(&restored, &restored_positions).unwrap();
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
}
