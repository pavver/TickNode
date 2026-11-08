//! Shared hub access, host environment adapter and core-client logging.

use std::cell::RefCell;
use std::sync::Once;
use ticknode_plugin_sdk::*;
use crate::hub::{Env, Hub};

thread_local! {
    /// There is one plugin instance, so the connection and the subscriptions live in static state.
    static HUB: RefCell<Hub> = RefCell::new(Hub::default());
}

pub(super) fn with_hub<R>(f: impl FnOnce(&mut Hub, &mut HostEnv) -> R) -> R {
    HUB.with(|hub| f(&mut hub.borrow_mut(), &mut HostEnv))
}

/// Like [`with_hub`], but silent if the hub is busy or gone (used from `Drop`).
pub(super) fn try_with_hub(f: impl FnOnce(&mut Hub, &mut HostEnv)) {
    let _ = HUB.try_with(|hub| {
        if let Ok(mut hub) = hub.try_borrow_mut() {
            f(&mut hub, &mut HostEnv);
        }
    });
}

pub(super) struct HostEnv;

impl Env for HostEnv {
    fn ws_connect(&mut self, url: &str) -> Result<WsId, String> {
        host::ws_connect(url, &[])
    }

    fn ws_send(&mut self, id: WsId, text: &str) -> Result<(), String> {
        host::ws_send(id, text)
    }

    fn ws_close(&mut self, id: WsId) {
        host::ws_close(id)
    }

    fn info(&mut self, message: &str) {
        host::log(message)
    }

    fn warn(&mut self, message: &str) {
        host::warn(message)
    }

    fn error(&mut self, message: &str) {
        host::error(message)
    }

    fn set_options(&mut self, source: &str, options: &[PropertyOption]) {
        #[cfg(target_arch = "wasm32")]
        host::set_options(source, options);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (source, options);
    }
}

/// Forwards warnings and errors of `ha-core-client` (its `log` facade) to the host log.
struct CoreLogger;

impl log::Log for CoreLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record) {
        let message = format!("ha-core-client: {}", record.args());
        match record.level() {
            log::Level::Error => host::error(&message),
            _ => host::warn(&message),
        }
    }

    fn flush(&self) {}
}

pub(super) fn install_core_logger() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if log::set_logger(&CoreLogger).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
        }
    });
}
