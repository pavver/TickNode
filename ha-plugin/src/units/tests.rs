use super::*;

#[test]
fn ha_symbols_use_the_catalog_and_preserve_case() {
    for (name, symbols) in [
        ("power", "W kW mW MW"),
        ("energy", "Wh kWh MWh"),
        ("voltage", "V mV"),
        ("current", "A mA"),
        ("apparent_power", "VA"),
        ("frequency", "Hz"),
        ("temperature", "°C °F K"),
        ("percentage", "%"),
        ("pressure", "hPa mbar bar kPa Pa psi mmHg inHg"),
        ("speed", "m/s km/h mph kn ft/s"),
        ("length", "mm cm m km in ft yd mi"),
        ("volume", "L mL m³ gal ft³ m3 m^3 ft^3"),
        ("mass", "g kg mg lb oz"),
        ("illuminance", "lx"),
        ("irradiance", "W/m²"),
        ("concentration", "µg/m³ μg/m³ mg/m³ µg/m3 μg/m3"),
        ("parts", "ppm ppb"),
    ] {
        for symbol in symbols.split_whitespace() {
            assert_eq!(
                mapping(symbol, None).unwrap().0,
                format!("quantity.{name}"),
                "{symbol}"
            );
        }
    }
    assert_eq!(mapping("mW", None).unwrap().1.factor, 0.001);
    assert_eq!(mapping("MW", None).unwrap().1.factor, 1e6);
    assert!(mapping("mw", None).is_none(), "ambiguous case folding");
    assert!(mapping("unknown", None).is_none());
    assert!(mapping("", None).is_none());
}

#[test]
fn device_classes_constrain_the_quantity_without_guessing_incompatible_units() {
    for (class, symbol, name) in [
        ("power", "kW", "power"),
        ("energy", "kWh", "energy"),
        ("voltage", "V", "voltage"),
        ("current", "mA", "current"),
        ("apparent_power", "VA", "apparent_power"),
        ("frequency", "Hz", "frequency"),
        ("temperature", "°F", "temperature"),
        ("battery", "%", "percentage"),
        ("humidity", "%", "percentage"),
        ("moisture", "%", "percentage"),
        ("pressure", "Pa", "pressure"),
        ("atmospheric_pressure", "hPa", "pressure"),
        ("speed", "m/s", "speed"),
        ("wind_speed", "km/h", "speed"),
        ("distance", "m", "length"),
        ("volume", "L", "volume"),
        ("water", "m³", "volume"),
        ("gas", "ft³", "volume"),
        ("weight", "lb", "mass"),
        ("illuminance", "lx", "illuminance"),
        ("irradiance", "W/m²", "irradiance"),
        ("pm25", "μg/m³", "concentration"),
        ("pm10", "mg/m³", "concentration"),
        ("pm1", "µg/m³", "concentration"),
        ("carbon_dioxide", "ppm", "parts"),
    ] {
        assert_eq!(
            mapping(symbol, Some(class)).unwrap().0,
            format!("quantity.{name}")
        );
    }
    assert!(mapping("W", Some("energy")).is_none());
    assert!(mapping("mw", Some("power")).is_none());
    assert!(mapping("unknown", Some("temperature")).is_none());
    assert_eq!(
        mapping("V", Some("future_class")).unwrap().0,
        "quantity.voltage"
    );
}
