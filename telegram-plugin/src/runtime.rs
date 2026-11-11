//! WASI adapters. Native tests inject transport, storage and clocks instead.
use crate::hub::{Env, Hub};
use std::cell::RefCell;
use ticknode_plugin_sdk::*;
thread_local! { static HUB: RefCell<Hub> = RefCell::new(Hub::default()); }
pub fn with<R>(f: impl FnOnce(&mut Hub, &mut HostEnv) -> R) -> R {
    HUB.with(|hub| f(&mut hub.borrow_mut(), &mut HostEnv))
}
pub fn remove(node: NodeId, serial: u64) {
    let _ = HUB.try_with(|hub| {
        if let Ok(mut hub) = hub.try_borrow_mut() {
            hub.remove(node, serial);
        }
    });
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub struct HostEnv;
impl Env for HostEnv {
    fn http(&mut self, request: HttpRequest) -> Result<TaskId, String> {
        host::http(request)
    }
    fn delay(&mut self, seconds: u64) -> Result<TaskId, String> {
        host::delay_ms(seconds.saturating_mul(1000))
    }
    fn cancel(&mut self, id: TaskId) {
        host::cancel(id);
    }
    fn log(&mut self, text: &str) {
        host::log(text);
    }
    fn settings_error(&mut self, error: &UiError) {
        host::node_error(error, true);
    }
    fn options(&mut self, source: &str, options: &[PropertyOption]) {
        host::set_options(source, options);
    }
    fn read(&mut self, name: &str) -> Result<Option<Vec<u8>>, String> {
        match std::fs::read(name) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(format!("Could not read Telegram {name}")),
        }
    }
    fn write(&mut self, name: &str, data: &[u8]) -> Result<(), String> {
        use std::io::Write;
        let path = format!("{name}.tmp");
        let save = || -> std::io::Result<()> {
            let mut file = std::fs::File::create(&path)?;
            file.write_all(data)?;
            file.sync_all()?;
            std::fs::rename(&path, name)
        };
        save().map_err(|_| format!("Could not persist Telegram {name}"))
    }
}
