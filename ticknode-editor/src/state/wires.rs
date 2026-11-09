//! Wire layout state; only drag previews mutate it before a server event.

use super::*;

pub type WireKey = (bool, ConnectionModel);
pub type WireRoutes = HashMap<WireKey, Vec<Position>>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WireContext {
    pub exec: bool,
    pub connection: ConnectionModel,
    pub index: Option<usize>,
    pub world: (f64, f64),
    pub screen: (f64, f64),
}

impl Editor {
    pub fn wire_exists(&self, (exec, connection): WireKey) -> bool {
        if exec { self.exec.with(|all| all.contains(&connection)) }
        else { self.data.with(|all| all.contains(&connection)) }
    }

    pub fn wire_exists_untracked(&self, (exec, connection): WireKey) -> bool {
        if exec { self.exec.with_untracked(|all| all.contains(&connection)) }
        else { self.data.with_untracked(|all| all.contains(&connection)) }
    }
}
