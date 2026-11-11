use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use ticknode_core::TickEngine;
use ticknode_shared::{Frame, FrameId, GraphModel, NodeId, NodeSize, Position, ServerSettings, SheetId, SheetInfo, Variable, VariableScope};
use tracing::{info, warn};
use crate::actor::routes::{self, Routes};

#[derive(Debug, Serialize, Deserialize)]
pub struct VariablesFile {
    pub variables: Vec<Variable>,
}

#[derive(Debug, Deserialize)]
struct SettingsFile {
    #[serde(default)]
    display: ticknode_shared::DisplayRules,
    #[serde(default = "default_settings_locale")]
    locale: String,
    tick_ms: Option<u32>,
    tick_budget: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SheetsFile {
    pub sheets: Vec<SheetInfo>,
}

pub struct LoadedSheets {
    pub sheets: Vec<SheetInfo>,
    pub node_sheet: HashMap<NodeId, SheetId>,
    pub frame_sheet: HashMap<FrameId, SheetId>,
    pub positions: HashMap<NodeId, Position>,
    pub sizes: HashMap<NodeId, NodeSize>,
    pub routes: Routes,
    pub frames: BTreeMap<FrameId, Frame>,
}

impl Default for LoadedSheets {
    fn default() -> Self {
        let main = SheetInfo {
            id: SheetId::generate(),
            title: "Main".to_string(),
            caption: String::new(),
            enabled: true,
        };
        Self {
            sheets: vec![main],
            node_sheet: HashMap::new(),
            frame_sheet: HashMap::new(),
            positions: HashMap::new(),
            sizes: HashMap::new(),
            routes: Routes::new(),
            frames: BTreeMap::new(),
        }
    }
}

/// The sheets metadata and individual sheet graphs are stored as pretty-printed JSON.
pub struct Storage {
    data_dir: PathBuf,
}

impl Storage {
    pub fn new(path_or_dir: impl Into<PathBuf>) -> Self {
        let path_or_dir = path_or_dir.into();
        let data_dir = if path_or_dir.extension().is_some() {
            path_or_dir.parent().unwrap_or(&path_or_dir).to_path_buf()
        } else {
            path_or_dir
        };
        Self { data_dir }
    }

    pub fn sheets_json_path(&self) -> PathBuf {
        self.data_dir.join("sheets.json")
    }

    pub fn sheets_dir(&self) -> PathBuf {
        self.data_dir.join("sheets")
    }

    pub fn sheet_path(&self, id: SheetId) -> PathBuf {
        self.sheets_dir().join(format!("{}.json", id.0))
    }

    pub fn variables_json_path(&self) -> PathBuf {
        self.data_dir.join("variables.json")
    }

    pub fn settings_json_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    pub fn load_settings(&self) -> anyhow::Result<ServerSettings> {
        Self::load_settings_from(&self.settings_json_path(), &|k| std::env::var(k).ok())
    }

    pub fn load_settings_from(path: &Path, env: &dyn Fn(&str) -> Option<String>) -> anyhow::Result<ServerSettings> {
        let (file_tick_ms, file_tick_budget, locale, display) = if path.exists() {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let file: SettingsFile = serde_json::from_str(&text)
                .with_context(|| format!("invalid settings JSON in {}", path.display()))?;
            (file.tick_ms, file.tick_budget, file.locale, file.display)
        } else {
            (None, None, "en".into(), Default::default())
        };

        let tick_ms = file_tick_ms
            .or_else(|| env("TICKNODE_TICK_MS").and_then(|v| v.parse().ok()))
            .unwrap_or(ticknode_shared::DEFAULT_TICK_MS);

        let tick_budget = file_tick_budget
            .or_else(|| env("TICKNODE_TICK_BUDGET").and_then(|v| v.trim().parse().ok()))
            .unwrap_or(ticknode_shared::DEFAULT_TICK_BUDGET);

        if !["en", "uk"].contains(&locale.as_str()) { tracing::warn!(%locale, "Invalid formatting locale; using en"); }
        display.validate().map_err(|e| anyhow::anyhow!(e.fallback))?;
        Ok(ServerSettings { tick_ms, tick_budget, locale, display }.validated())
    }

    pub fn save_settings(&self, settings: &ServerSettings) -> anyhow::Result<()> {
        write_atomic(&self.settings_json_path(), &(serde_json::to_string_pretty(settings)? + "\n"))
    }

    pub fn load_variables(&self) -> anyhow::Result<Vec<Variable>> {
        let path = self.variables_json_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let file: VariablesFile = serde_json::from_str(&text)
            .with_context(|| format!("invalid variables JSON in {}", path.display()))?;
        Ok(file.variables)
    }

    pub fn save_variables(&self, variables: &[Variable]) -> anyhow::Result<()> {
        let mut sorted: Vec<_> = variables.iter().filter(|v| v.key.scope == VariableScope::Persistent).cloned().collect();
        sorted.sort_by(|a, b| a.key.name.cmp(&b.key.name));
        let file = VariablesFile { variables: sorted };
        let path = self.variables_json_path();
        crate::plugins::config::write_private(&self.data_dir, &path, &(serde_json::to_string_pretty(&file)? + "\n"))
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.sheets_dir().join("trash")
    }

    pub fn load(&self, engine: &mut TickEngine) -> anyhow::Result<LoadedSheets> {
        self.ensure_migrated()?;

        let sheets_text = std::fs::read_to_string(self.sheets_json_path())
            .with_context(|| format!("failed to read {}", self.sheets_json_path().display()))?;
        let sheets_file: SheetsFile = serde_json::from_str(&sheets_text)
            .with_context(|| format!("invalid sheets JSON in {}", self.sheets_json_path().display()))?;

        let mut node_sheet = HashMap::new();
        let mut frame_sheet = HashMap::new();
        let mut merged_graph = GraphModel::default();

        for sheet in &sheets_file.sheets {
            let sheet_path = self.sheet_path(sheet.id);
            let graph: GraphModel = if sheet_path.exists() {
                let text = std::fs::read_to_string(&sheet_path)
                    .with_context(|| format!("failed to read {}", sheet_path.display()))?;
                serde_json::from_str(&text)
                    .with_context(|| format!("invalid sheet JSON in {}", sheet_path.display()))?
            } else {
                GraphModel::default()
            };

            for node in &graph.nodes {
                node_sheet.insert(node.id, sheet.id);
                engine.set_node_sheet(node.id, Some(sheet.id));
            }
            for frame in &graph.frames {
                frame_sheet.insert(frame.id, sheet.id);
            }

            merged_graph.nodes.extend(graph.nodes);
            merged_graph.exec_connections.extend(graph.exec_connections);
            merged_graph.data_connections.extend(graph.data_connections);
            merged_graph.wire_routes.extend(graph.wire_routes);
            merged_graph.detached_connections.extend(graph.detached_connections);
            merged_graph.frames.extend(graph.frames);
        }

        for warning in engine.load_graph(&merged_graph) {
            warn!("{}", warning);
        }

        let persistent_vars = self.load_variables()?;
        for var in persistent_vars {
            engine.variables.create(var.key, var.data_type, var.value);
        }
        engine.variables.clear_persistent_dirty();
        engine.variables.take_changes();

        for sheet in &sheets_file.sheets {
            if !sheet.enabled {
                let disabled = node_sheet.iter()
                    .filter(|(_, s)| **s == sheet.id)
                    .map(|(n, _)| *n);
                engine.set_disabled(disabled, true);
            }
        }

        let mut frames = BTreeMap::new();
        for frame in merged_graph.frames {
            let id = frame.id;
            if let Some(valid) = frame.validated() {
                frames.insert(valid.id, valid);
            } else {
                warn!("Dropping invalid frame {}: non-finite position or size", id.0);
            }
        }

        let positions = merged_graph.nodes.iter().map(|n| (n.id, n.position)).collect();
        let sizes = merged_graph.nodes.iter().filter_map(|n| Some((n.id, n.size?.validated()?))).collect();
        let routes = routes::from_routes(engine, &merged_graph.wire_routes);

        info!(
            sheets = sheets_file.sheets.len(),
            nodes = merged_graph.nodes.len(),
            frames = frames.len(),
            "Sheets and graph loaded"
        );

        Ok(LoadedSheets {
            sheets: sheets_file.sheets,
            node_sheet,
            frame_sheet,
            positions,
            sizes,
            routes,
            frames,
        })
    }

    fn ensure_migrated(&self) -> anyhow::Result<()> {
        let sheets_json = self.sheets_json_path();
        if sheets_json.exists() {
            return Ok(());
        }

        let old_graph = self.data_dir.join("graph.json");
        let (graph, should_migrate_old) = if old_graph.exists() {
            let text = std::fs::read_to_string(&old_graph)
                .with_context(|| format!("failed to read {}", old_graph.display()))?;
            let g: GraphModel = serde_json::from_str(&text)
                .with_context(|| format!("invalid graph JSON in {}", old_graph.display()))?;
            (g, true)
        } else {
            (GraphModel::default(), false)
        };

        std::fs::create_dir_all(self.sheets_dir())
            .with_context(|| format!("failed to create sheets dir {}", self.sheets_dir().display()))?;

        let main_id = SheetId::generate();
        let main_sheet = SheetInfo {
            id: main_id,
            title: "Main".to_string(),
            caption: String::new(),
            enabled: true,
        };

        let sheets_file = SheetsFile {
            sheets: vec![main_sheet],
        };

        let sheet_path = self.sheet_path(main_id);
        write_atomic(&sheet_path, &(serde_json::to_string_pretty(&graph)? + "\n"))?;
        write_atomic(&sheets_json, &(serde_json::to_string_pretty(&sheets_file)? + "\n"))?;

        if should_migrate_old {
            let migrated = self.data_dir.join("graph.json.migrated");
            std::fs::rename(&old_graph, &migrated)
                .with_context(|| format!("failed to rename {} to {}", old_graph.display(), migrated.display()))?;
            info!("Migrated graph.json to sheets layout and renamed to graph.json.migrated");
        }

        Ok(())
    }

    pub fn save_sheets_meta(&self, sheets: &[SheetInfo]) -> anyhow::Result<()> {
        let file = SheetsFile { sheets: sheets.to_vec() };
        write_atomic(&self.sheets_json_path(), &(serde_json::to_string_pretty(&file)? + "\n"))
    }

    pub fn save_sheet(&self, id: SheetId, graph: &GraphModel) -> anyhow::Result<()> {
        std::fs::create_dir_all(self.sheets_dir())?;
        write_atomic(&self.sheet_path(id), &(serde_json::to_string_pretty(graph)? + "\n"))
    }

    pub fn delete_sheet(&self, id: SheetId) -> anyhow::Result<()> {
        let src = self.sheet_path(id);
        if src.exists() {
            let trash_dir = self.trash_dir();
            std::fs::create_dir_all(&trash_dir)?;
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let dest = trash_dir.join(format!("{}-{}.json", id.0, now_secs));
            std::fs::rename(&src, &dest)
                .with_context(|| format!("failed to move {} to trash {}", src.display(), dest.display()))?;
        }
        Ok(())
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
mod tests;

fn default_settings_locale() -> String { "en".into() }
