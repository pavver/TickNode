//! Hash-based routing for editor screens: home page (`#/`) and sheet page (`#/sheet/<uuid>`).

use ticknode_shared::SheetId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Home,
    Sheet(SheetId),
}

/// Parses the URL hash into a `Route`. Empty hash, `/`, or unrecognized paths fall back to `Home`.
pub fn parse_route(hash: &str) -> Route {
    let s = hash.trim().trim_start_matches('#');
    let s = s.strip_prefix('!').unwrap_or(s);
    let s = s.trim_start_matches('/');
    if s.is_empty() {
        return Route::Home;
    }
    if let Some(rest) = s.strip_prefix("sheet/") {
        let rest = rest.trim_end_matches('/');
        if let Ok(uuid) = ticknode_shared::Uuid::parse_str(rest) {
            return Route::Sheet(SheetId(uuid));
        }
    }
    Route::Home
}

/// Converts a `Route` to its URL hash representation.
pub fn route_hash(route: &Route) -> String {
    match route {
        Route::Home => "#/".to_string(),
        Route::Sheet(id) => format!("#/sheet/{}", id.0),
    }
}

/// Reads the current route from `window().location().hash()`.
pub fn current_route() -> Route {
    let hash = leptos::prelude::window()
        .location()
        .hash()
        .unwrap_or_default();
    parse_route(&hash)
}

/// Sets the URL hash to the given route. If the hash is already set to this route,
/// dispatches a synthetic `hashchange` event so listeners always react.
pub fn navigate(route: &Route) {
    let hash = route_hash(route);
    let loc = leptos::prelude::window().location();
    if loc.hash().as_deref() == Ok(&hash) {
        if let Some(win) = web_sys::window() {
            if let Ok(event) = web_sys::Event::new("hashchange") {
                let _ = win.dispatch_event(&event);
            }
        }
        return;
    }
    let _ = loc.set_hash(&hash);
}

#[cfg(test)]
mod tests;
