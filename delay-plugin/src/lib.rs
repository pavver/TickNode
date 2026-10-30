//! Delays with different retrigger behaviors. The delay is specified in milliseconds
//! and rounded up to whole ticks.

use ticknode_plugin_sdk::*;

struct DelayPlugin;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Each pulse starts its own independent delay.
    Queue,
    /// Pulses arriving during an active delay are ignored.
    Ignore,
    /// A new pulse cancels the active delay and restarts the countdown.
    Reset,
}

struct DelayNode {
    mode: Mode,
    /// Latest active delay task (for Ignore/Reset).
    active: Option<TaskId>,
}

impl DelayNode {
    /// (type, title, description, base pin ID: +0 In, +1 Out, +2 Delay).
    fn info(&self) -> (&'static str, &'static str, &'static str, u32) {
        match self.mode {
            Mode::Queue => ("delay/queue", "Queue Delay", "Кожен імпульс запускає незалежну затримку", 0),
            Mode::Ignore => ("delay/ignore", "Ignore Delay", "Ігнорує нові імпульси, поки затримка триває", 10),
            Mode::Reset => ("delay/reset", "Reset Delay", "Новий імпульс скидає затримку й починає відлік заново", 20),
        }
    }
}

impl PluginNode for DelayNode {
    fn definition(&self) -> NodeDefinition {
        let (type_name, title, description, base) = self.info();
        NodeDefinition {
            type_name: type_name.into(),
            title: title.into(),
            category: "Time".into(),
            description: description.into(),
            help: match self.mode {
                Mode::Queue => "Кожен сигнал на In запускає власну затримку на «Delay ms» мілісекунд, а по її завершенні видає сигнал на Out.\n\nКілька сигналів поспіль дадуть стільки ж сигналів на виході з тими самими інтервалами.",
                Mode::Ignore => "Сигнал на In запускає затримку на «Delay ms» мілісекунд, а по її завершенні видає сигнал на Out. Поки затримка триває, нові сигнали ігноруються.\n\nКорисно, щоб не реагувати на часті повтори події.",
                Mode::Reset => "Кожен сигнал на In скасовує поточну затримку й починає відлік заново. Out спрацює лише після паузи без сигналів довжиною «Delay ms».\n\nНаприклад: вимкнути світло через 5 хвилин після останнього руху.",
            }.to_string() + "\n\nЗатримка округлюється вгору до цілих тактів (такт сервера — 100 мс).",
            pins: vec![
                exec_in(base, "In"),
                data_in(base + 2, "Delay ms", DataType::Number),
                exec_out(base + 1, "Out"),
            ],
        }
    }

    fn execute(&mut self, trigger: Trigger, inputs: &Inputs) -> Vec<Output> {
        let (_, _, _, base) = self.info();
        match trigger {
            Trigger::Pin(pin) if pin == PinId(base) => {
                if self.mode == Mode::Ignore && self.active.is_some() {
                    return Vec::new();
                }
                if let (Mode::Reset, Some(task)) = (self.mode, self.active.take()) {
                    host::cancel(task);
                }
                let ms = inputs.number(PinId(base + 2), 0.0).max(0.0) as u64;
                match host::delay_ms(ms) {
                    Ok(task) => self.active = Some(task),
                    Err(e) => host::error(&e),
                }
                Vec::new()
            }
            Trigger::Task { id, .. } => {
                if self.active == Some(id) {
                    self.active = None;
                }
                vec![Output::Exec(PinId(base + 1))]
            }
            _ => Vec::new(),
        }
    }
}

impl Plugin for DelayPlugin {
    const NODE_TYPES: &'static [&'static str] = &["delay/queue", "delay/ignore", "delay/reset"];

    fn manifest() -> PluginManifest {
        PluginManifest {
            name: "delay_plugin".into(),
            version: "0.2.0".into(),
            author: "TickNode".into(),
            requested_capabilities: vec![],
            settings: vec![],
        }
    }

    fn create(type_name: &str) -> Option<Box<dyn PluginNode>> {
        let mode = match type_name {
            "delay/queue" => Mode::Queue,
            "delay/ignore" => Mode::Ignore,
            "delay/reset" => Mode::Reset,
            _ => return None,
        };
        Some(Box::new(DelayNode { mode, active: None }))
    }
}

export_plugin!(DelayPlugin);
