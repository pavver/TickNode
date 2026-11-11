//! Sensor scales and energy use per distance are separate canonical numeric quantities.

use super::catalog::{quantity, with_decimals};
use crate::TypeDefinition;

#[rustfmt::skip]
pub(super) fn quantities() -> Vec<TypeDefinition> {
    vec![
        with_decimals(quantity("beaufort", "Wind force (Beaufort)", &[
            ("beaufort", "Bft", 1.0, 0.0, &["Beaufort", "Бфт"]),
        ], &["beaufort"]), 0),
        quantity("energy_consumption", "Energy consumption per distance", &[
            ("kilowatt_hour_per_100_kilometers", "kWh/100km", 1.0, 0.0, &["kilowatt_hour_per_100_kilometers", "кВт·год/100км"]),
            ("watt_hour_per_kilometer", "Wh/km", 0.1, 0.0, &["watt_hour_per_kilometer", "Вт·год/км"]),
        ], &["kilowatt_hour_per_100_kilometers"]),
        quantity("energy_efficiency", "Distance per energy", &[
            ("kilometer_per_kilowatt_hour", "km/kWh", 1.0, 0.0, &["kilometer_per_kilowatt_hour", "км/кВт·год"]),
            ("mile_per_kilowatt_hour", "mi/kWh", 1.609344, 0.0, &["mile_per_kilowatt_hour", "миль/кВт·год"]),
        ], &["kilometer_per_kilowatt_hour"]),
        with_decimals(quantity("aqi", "Air quality index", &[
            ("aqi", "AQI", 1.0, 0.0, &["aqi"]),
        ], &["aqi"]), 0),
        with_decimals(quantity("ph", "pH", &[
            ("ph", "pH", 1.0, 0.0, &["ph"]),
        ], &["ph"]), 1),
    ]
}
