use crate::engine::RemovedConnections;
use std::borrow::Borrow;
use std::collections::HashMap;
use ticknode_shared::{ConnectionModel, NodeId, PinId};

/// Index storing exec and data connections with fast lookups for hot execution paths.
#[derive(Debug, Default, Clone)]
pub(crate) struct ConnectionIndex {
    exec: Vec<ConnectionModel>,
    data: Vec<ConnectionModel>,
    exec_out: HashMap<(NodeId, PinId), Vec<ConnectionModel>>,
    data_in: HashMap<NodeId, Vec<ConnectionModel>>,
}

impl ConnectionIndex {
    pub fn exec(&self) -> &[ConnectionModel] {
        &self.exec
    }

    pub fn data(&self) -> &[ConnectionModel] {
        &self.data
    }

    pub fn contains_exec(&self, c: impl Borrow<ConnectionModel>) -> bool {
        let c = c.borrow();
        self.exec_out
            .get(&(c.src_node, c.src_pin))
            .map_or(false, |list| list.contains(c))
    }

    pub fn contains_data(&self, c: impl Borrow<ConnectionModel>) -> bool {
        let c = c.borrow();
        self.data_in
            .get(&c.dst_node)
            .map_or(false, |list| list.contains(c))
    }

    pub fn add_exec(&mut self, c: ConnectionModel) {
        if !self.contains_exec(&c) {
            self.exec.push(c);
            self.exec_out
                .entry((c.src_node, c.src_pin))
                .or_default()
                .push(c);
        }
    }

    pub fn remove_exec(&mut self, c: impl Borrow<ConnectionModel>) -> bool {
        let c = c.borrow();
        let Some(list) = self.exec_out.get_mut(&(c.src_node, c.src_pin)) else {
            return false;
        };
        let Some(pos) = list.iter().position(|x| x == c) else {
            return false;
        };
        list.remove(pos);
        if list.is_empty() {
            self.exec_out.remove(&(c.src_node, c.src_pin));
        }
        let exec_pos = self
            .exec
            .iter()
            .position(|x| x == c)
            .expect("exec index in sync");
        self.exec.remove(exec_pos);
        true
    }

    /// Connects a data link. Since a data input has at most one source, replaces and
    /// returns the previous connection to the same `(dst_node, dst_pin)`, if any.
    pub fn set_data(&mut self, c: ConnectionModel) -> Option<ConnectionModel> {
        let old = self.remove_data_input(c.dst_node, c.dst_pin);
        self.data.push(c);
        self.data_in.entry(c.dst_node).or_default().push(c);
        old
    }

    /// Removes the data connection targeting `(dst_node, dst_pin)`, returning it if present.
    pub fn remove_data_input(
        &mut self,
        dst_node: NodeId,
        dst_pin: PinId,
    ) -> Option<ConnectionModel> {
        let list = self.data_in.get_mut(&dst_node)?;
        let in_pos = list.iter().position(|x| x.dst_pin == dst_pin)?;
        let old = list.remove(in_pos);
        if list.is_empty() {
            self.data_in.remove(&dst_node);
        }
        let pos = self
            .data
            .iter()
            .position(|x| *x == old)
            .expect("data index in sync");
        self.data.remove(pos);
        Some(old)
    }

    /// Filters exec and data connections in place, returning what was dropped and rebuilding indices.
    pub fn retain(
        &mut self,
        mut keep_exec: impl FnMut(&ConnectionModel) -> bool,
        mut keep_data: impl FnMut(&ConnectionModel) -> bool,
    ) -> RemovedConnections {
        let mut dropped_exec = Vec::new();
        let mut retained_exec = Vec::with_capacity(self.exec.len());
        for c in self.exec.drain(..) {
            if keep_exec(&c) {
                retained_exec.push(c);
            } else {
                dropped_exec.push(c);
            }
        }
        self.exec = retained_exec;

        let mut dropped_data = Vec::new();
        let mut retained_data = Vec::with_capacity(self.data.len());
        for c in self.data.drain(..) {
            if keep_data(&c) {
                retained_data.push(c);
            } else {
                dropped_data.push(c);
            }
        }
        self.data = retained_data;

        self.rebuild_indices();

        RemovedConnections {
            exec: dropped_exec,
            data: dropped_data,
        }
    }

    fn rebuild_indices(&mut self) {
        self.exec_out.clear();
        for c in &self.exec {
            self.exec_out
                .entry((c.src_node, c.src_pin))
                .or_default()
                .push(*c);
        }
        self.data_in.clear();
        for c in &self.data {
            self.data_in.entry(c.dst_node).or_default().push(*c);
        }
    }

    pub fn exec_successors(&self, node: NodeId, pin: PinId) -> &[ConnectionModel] {
        self.exec_out
            .get(&(node, pin))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn data_into(&self, node: NodeId) -> &[ConnectionModel] {
        self.data_in.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn data_source(&self, node: NodeId, pin: PinId) -> Option<ConnectionModel> {
        self.data_in
            .get(&node)?
            .iter()
            .find(|c| c.dst_pin == pin)
            .copied()
    }
}

#[cfg(test)]
mod tests;
