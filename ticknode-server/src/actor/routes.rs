//! Editor-only wire layout, validation and restoration.

use std::collections::{HashMap, HashSet};
use ticknode_core::TickEngine;
use ticknode_shared::{
    ConnectionModel, MAX_WIRE_POINTS, Position, ServerMessage, SheetId, UiMessage, WireRoute,
};

use super::Actor;

pub(crate) type Routes = HashMap<(bool, ConnectionModel), Vec<Position>>;

pub(crate) fn connections(engine: &TickEngine) -> HashSet<(bool, ConnectionModel)> {
    engine
        .exec_connections()
        .iter()
        .map(|c| (true, *c))
        .chain(engine.data_connections().iter().map(|c| (false, *c)))
        .collect()
}

pub(crate) fn valid_points(points: &[Position]) -> bool {
    points.len() <= MAX_WIRE_POINTS && points.iter().all(|p| p.x.is_finite() && p.y.is_finite())
}

pub(crate) fn sorted(routes: &Routes) -> Vec<WireRoute> {
    let mut result: Vec<_> = routes
        .iter()
        .map(|((exec, connection), points)| WireRoute {
            exec: *exec,
            connection: *connection,
            points: points.clone(),
        })
        .collect();
    result.sort_by_key(|r| {
        (
            r.exec,
            r.connection.src_node,
            r.connection.src_pin.0,
            r.connection.dst_node,
            r.connection.dst_pin.0,
        )
    });
    result
}

pub(crate) fn from_routes(engine: &TickEngine, routes: &[WireRoute]) -> Routes {
    if routes.is_empty() {
        return Routes::new();
    }
    let existing = connections(engine);
    routes
        .iter()
        .filter(|r| {
            existing.contains(&(r.exec, r.connection))
                && !r.points.is_empty()
                && valid_points(&r.points)
        })
        .map(|r| ((r.exec, r.connection), r.points.clone()))
        .collect()
}

impl Actor {
    pub(super) fn set_wire_route(
        &mut self,
        route: WireRoute,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_connection(&route.connection, sheet)?;
        let existing = if route.exec {
            self.engine.exec_connections()
        } else {
            self.engine.data_connections()
        };
        if !existing.contains(&route.connection) || !valid_points(&route.points) {
            return Err(UiMessage::new(
                "error-wire-route",
                "Invalid wire route: the connection must exist and the route must contain at most 32 finite points",
            ));
        }
        let key = (route.exec, route.connection);
        if self.routes.get(&key).map(Vec::as_slice).unwrap_or_default() == route.points {
            return Ok(vec![]);
        }
        if route.points.is_empty() {
            self.routes.remove(&key);
        } else {
            self.routes.insert(key, route.points.clone());
        }
        Ok(vec![ServerMessage::WireRouted(route)])
    }

    pub(super) fn prune_routes(&mut self) {
        if self.routes.is_empty() {
            return;
        }
        let existing = connections(&self.engine);
        self.routes.retain(|key, _| existing.contains(key));
    }

    pub(super) fn restore_routes_for_sheet(
        &mut self,
        sheet: SheetId,
        target: &[WireRoute],
    ) -> Vec<ServerMessage> {
        let restored = from_routes(&self.engine, target);
        let mut changes = Routes::new();
        for (key, points) in &restored {
            if self.routes.get(key) != Some(points) {
                changes.insert(*key, points.clone());
                self.routes.insert(*key, points.clone());
            }
        }
        let sheet_keys: Vec<_> = self
            .routes
            .keys()
            .filter(|(_, conn)| self.node_sheet.get(&conn.src_node) == Some(&sheet))
            .copied()
            .collect();
        for key in sheet_keys {
            if !restored.contains_key(&key) {
                changes.insert(key, Vec::new());
                self.routes.remove(&key);
            }
        }
        sorted(&changes)
            .into_iter()
            .map(ServerMessage::WireRouted)
            .collect()
    }

    pub(super) fn paste_routes(
        &mut self,
        routes: &[WireRoute],
        imported: &[(ticknode_shared::NodeId, ticknode_shared::NodeId)],
    ) -> Vec<ServerMessage> {
        let ids: HashMap<_, _> = imported.iter().copied().collect();
        let remapped: Vec<_> = routes
            .iter()
            .filter_map(|r| {
                Some(WireRoute {
                    exec: r.exec,
                    connection: ConnectionModel {
                        src_node: *ids.get(&r.connection.src_node)?,
                        dst_node: *ids.get(&r.connection.dst_node)?,
                        ..r.connection
                    },
                    points: r.points.clone(),
                })
            })
            .collect();
        let valid = from_routes(&self.engine, &remapped);
        self.routes.extend(valid.clone());
        sorted(&valid)
            .into_iter()
            .map(ServerMessage::WireRouted)
            .collect()
    }
}
