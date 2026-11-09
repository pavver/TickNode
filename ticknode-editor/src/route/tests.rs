use super::*;
use ticknode_shared::Uuid;

#[test]
fn parse_route_handles_home_variants() {
    assert_eq!(parse_route(""), Route::Home);
    assert_eq!(parse_route("#"), Route::Home);
    assert_eq!(parse_route("#/"), Route::Home);
    assert_eq!(parse_route("#!/"), Route::Home);
    assert_eq!(parse_route("/"), Route::Home);
    assert_eq!(parse_route("  #/  "), Route::Home);
}

#[test]
fn parse_route_handles_sheet_variants() {
    let uuid_str = "67e55044-10b1-426f-9247-bb680e5fe0c8";
    let id = SheetId(Uuid::parse_str(uuid_str).unwrap());
    assert_eq!(parse_route(&format!("#/sheet/{uuid_str}")), Route::Sheet(id));
    assert_eq!(parse_route(&format!("#/sheet/{uuid_str}/")), Route::Sheet(id));
    assert_eq!(parse_route(&format!("sheet/{uuid_str}")), Route::Sheet(id));
    assert_eq!(parse_route(&format!("/sheet/{uuid_str}")), Route::Sheet(id));
}

#[test]
fn parse_route_falls_back_on_invalid_sheet_or_unknown_path() {
    assert_eq!(parse_route("#/sheet/not-a-uuid"), Route::Home);
    assert_eq!(parse_route("#/sheet/"), Route::Home);
    assert_eq!(parse_route("#/other/stuff"), Route::Home);
    assert_eq!(parse_route("#/random"), Route::Home);
}

#[test]
fn route_hash_produces_canonical_hash() {
    assert_eq!(route_hash(&Route::Home), "#/");
    let uuid_str = "67e55044-10b1-426f-9247-bb680e5fe0c8";
    let id = SheetId(Uuid::parse_str(uuid_str).unwrap());
    assert_eq!(route_hash(&Route::Sheet(id)), format!("#/sheet/{uuid_str}"));
}

#[test]
fn route_round_trip() {
    let routes = [
        Route::Home,
        Route::Sheet(SheetId(Uuid::nil())),
        Route::Sheet(SheetId::generate()),
    ];
    for route in routes {
        let hash = route_hash(&route);
        assert_eq!(parse_route(&hash), route);
    }
}
