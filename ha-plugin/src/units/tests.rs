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
fn device_classes_try_candidates_before_unambiguous_symbol_fallback() {
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
        ("absolute_humidity", "g/m³", "concentration"),
        ("area", "m²", "area"),
        ("power_factor", "%", "percentage"),
        ("blood_glucose_concentration", "mmol/L", "blood_glucose"),
        ("carbon_monoxide", "ppm", "parts"),
        ("conductivity", "µS/cm", "conductivity"),
        ("data_rate", "Mbit/s", "data_rate"),
        ("data_size", "GB", "data_size"),
        ("precipitation", "mm", "length"),
        ("energy_storage", "GJ", "energy"),
        ("volume_storage", "MCF", "volume"),
        ("nitrogen_dioxide", "µg/m³", "concentration"),
        ("nitrogen_monoxide", "µg/m³", "concentration"),
        ("nitrous_oxide", "µg/m³", "concentration"),
        ("ozone", "µg/m³", "concentration"),
        ("sulphur_dioxide", "µg/m³", "concentration"),
        ("volatile_organic_compounds", "µg/m³", "concentration"),
        ("volatile_organic_compounds_parts", "ppb", "parts"),
        ("precipitation_intensity", "mm/d", "precipitation_intensity"),
        ("reactive_power", "mvar", "reactive_power"),
        ("reactive_energy", "kvarh", "reactive_energy"),
        ("signal_strength", "dBm", "signal_strength"),
        ("sound_pressure", "dBA", "sound_pressure"),
        ("volume_flow_rate", "ft³/min", "volume_flow_rate"),
        ("wind_direction", "°", "angle"),
    ] {
        assert_eq!(
            mapping(symbol, Some(class)).unwrap().0,
            format!("quantity.{name}")
        );
    }
    assert_eq!(mapping("W", Some("energy")).unwrap().0, "quantity.power");
    assert!(mapping("mw", Some("power")).is_none());
    assert!(mapping("unknown", Some("temperature")).is_none());
    assert_eq!(
        mapping("V", Some("future_class")).unwrap().0,
        "quantity.voltage"
    );
}

#[test]
fn ambiguous_rate_units_are_resolved_by_device_class() {
    for symbol in ["in/d", "in/h", "mm/d"] {
        assert!(mapping(symbol, None).is_none());
        assert_eq!(mapping(symbol, Some("speed")).unwrap().0, "quantity.speed");
        assert_eq!(
            mapping(symbol, Some("precipitation_intensity")).unwrap().0,
            "quantity.precipitation_intensity"
        );
    }
    assert_eq!(
        mapping("mm/d", Some("speed")).unwrap().1.factor,
        0.001 / 86400.0
    );
    assert_eq!(
        mapping("mm/d", Some("precipitation_intensity"))
            .unwrap()
            .1
            .factor,
        1.0 / 24.0
    );
    assert_eq!(
        mapping("mvar", Some("reactive_power")).unwrap().1.factor,
        0.001
    );
    assert_eq!(
        mapping("dB", Some("signal_strength")).unwrap().0,
        "quantity.sound_pressure"
    );
}

#[test]
fn pollutant_classes_accept_both_mass_and_parts_concentrations() {
    for class in [
        "carbon_monoxide",
        "nitrogen_dioxide",
        "nitrogen_monoxide",
        "nitrous_oxide",
        "ozone",
        "sulphur_dioxide",
        "pm1",
        "pm25",
        "pm10",
        "volatile_organic_compounds",
    ] {
        for (symbol, expected) in [
            ("ppm", "parts"),
            ("ppb", "parts"),
            ("µg/m³", "concentration"),
        ] {
            assert_eq!(
                mapping(symbol, Some(class)).unwrap().0,
                format!("quantity.{expected}")
            );
        }
    }
    for symbol in ["µS/cm", "μS/cm", "uS/cm"] {
        assert_eq!(
            mapping(symbol, Some("conductivity")).unwrap().0,
            "quantity.conductivity"
        );
    }
    for symbol in ["BTU/(h⋅ft²)", "BTU/(h*ft2)"] {
        assert_eq!(
            mapping(symbol, Some("irradiance")).unwrap().0,
            "quantity.irradiance"
        );
    }
}
