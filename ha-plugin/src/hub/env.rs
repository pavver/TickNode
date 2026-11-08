//! Host operations used by the testable Home Assistant hub.

use ticknode_plugin_sdk::{PropertyOption, WsId};

/// What the hub needs from the host.
pub trait Env {
    fn ws_connect(&mut self, url: &str) -> Result<WsId, String>;
    fn ws_send(&mut self, id: WsId, text: &str) -> Result<(), String>;
    fn ws_close(&mut self, id: WsId);
    fn info(&mut self, message: &str);
    fn warn(&mut self, message: &str);
    fn error(&mut self, message: &str);
    /// Publishes the options of a dynamic property source (`host::set_options`).
    fn set_options(&mut self, source: &str, options: &[PropertyOption]);
}
