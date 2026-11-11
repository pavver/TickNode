//! Sensor scales and energy use per distance are separate canonical numeric quantities.

use super::UnitCurve;
use super::catalog::{quantity, with_curves, with_decimals};
use crate::TypeDefinition;

#[rustfmt::skip]
pub(super) fn quantities() -> Vec<TypeDefinition> {
    vec![
        with_decimals(quantity("beaufort", "Wind force (Beaufort)", &[
            ("beaufort", "Bft", 1.0, 0.0, &["Beaufort", "Бфт"]),
        ], &["beaufort"]), 0),
        with_curves(quantity("energy_consumption", "Energy per distance", &[
            ("kilowatt_hour_per_100_kilometers", "kWh/100km", 1.0, 0.0, &["kilowatt_hour_per_100_kilometers", "кВт·год/100км"]),
            ("watt_hour_per_kilometer", "Wh/km", 0.1, 0.0, &["watt_hour_per_kilometer", "Вт·год/км"]),
            ("kilometer_per_kilowatt_hour", "km/kWh", 100.0, 0.0, &["kilometer_per_kilowatt_hour", "км/кВт·год"]),
            ("mile_per_kilowatt_hour", "mi/kWh", 100.0 / 1.609344, 0.0, &["mile_per_kilowatt_hour", "миль/кВт·год"]),
        ], &["kilowatt_hour_per_100_kilometers"]), &[
            ("kilometer_per_kilowatt_hour", UnitCurve::Inverse),
            ("mile_per_kilowatt_hour", UnitCurve::Inverse),
        ]),
        with_decimals(with_curves(quantity("color_temperature", "Color temperature", &[
            ("kelvin", "K", 1.0, 0.0, &["kelvin", "Kelvin", "К"]),
            ("mired", "mired", 1e6, 0.0, &["mireds", "майред"]),
        ], &["kelvin"]), &[("mired", UnitCurve::Inverse)]), 0),
        with_decimals(quantity("aqi", "Air quality index", &[
            ("aqi", "AQI", 1.0, 0.0, &["aqi"]),
        ], &["aqi"]), 0),
        with_decimals(quantity("ph", "pH", &[
            ("ph", "pH", 1.0, 0.0, &["ph"]),
        ], &["ph"]), 1),
    ]
}
