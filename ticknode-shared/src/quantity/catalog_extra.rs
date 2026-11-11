//! Additional physical quantities for sensor and integration data.

use super::catalog::quantity;
use crate::TypeDefinition;

#[rustfmt::skip]
pub(super) fn quantities() -> Vec<TypeDefinition> {
    vec![
        quantity("reactive_power", "Reactive power", &[
            ("var", "var", 1.0, 0.0, &["var", "вар"]),
            ("kilovar", "kvar", 1000.0, 0.0, &["kilovar", "квар"]),
            ("millivar", "mvar", 0.001, 0.0, &["millivar", "мвар"]),
        ], &["millivar", "var", "kilovar"]),
        quantity("reactive_energy", "Reactive energy", &[
            ("varh", "varh", 1.0, 0.0, &["varh", "вар·год"]),
            ("kvarh", "kvarh", 1000.0, 0.0, &["kvarh", "квар·год"]),
        ], &["varh", "kvarh"]),
        quantity("area", "Area", &[
            ("square_meter", "m²", 1.0, 0.0, &["square_meter", "m2", "m^2", "м²"]),
            ("square_centimeter", "cm²", 1e-4, 0.0, &["square_centimeter", "cm2", "cm^2", "см²"]),
            ("square_millimeter", "mm²", 1e-6, 0.0, &["square_millimeter", "mm2", "mm^2", "мм²"]),
            ("square_kilometer", "km²", 1e6, 0.0, &["square_kilometer", "km2", "km^2", "км²"]),
            ("square_inch", "in²", 0.00064516, 0.0, &["square_inch", "in2", "in^2", "дюйм²"]),
            ("square_foot", "ft²", 0.09290304, 0.0, &["square_foot", "ft2", "ft^2", "фут²"]),
            ("square_yard", "yd²", 0.83612736, 0.0, &["square_yard", "yd2", "yd^2", "ярд²"]),
            ("square_mile", "mi²", 2589988.110336, 0.0, &["square_mile", "mi2", "mi^2", "миля²"]),
            ("acre", "ac", 4046.8564224, 0.0, &["acre", "акр"]),
            ("hectare", "ha", 10000.0, 0.0, &["hectare", "га"]),
        ], &["square_millimeter", "square_centimeter", "square_meter", "square_kilometer"]),
        quantity("data_rate", "Data rate", &[
            ("bit_per_second", "bit/s", 1.0, 0.0, &["bit_per_second", "біт/с"]),
            ("kilobit_per_second", "kbit/s", 1e3, 0.0, &["kilobit_per_second", "кбіт/с"]),
            ("megabit_per_second", "Mbit/s", 1e6, 0.0, &["megabit_per_second", "Мбіт/с"]),
            ("gigabit_per_second", "Gbit/s", 1e9, 0.0, &["gigabit_per_second", "Гбіт/с"]),
            ("byte_per_second", "B/s", 8.0, 0.0, &["byte_per_second", "Б/с"]),
            ("kilobyte_per_second", "kB/s", 8e3, 0.0, &["kilobyte_per_second", "кБ/с"]),
            ("megabyte_per_second", "MB/s", 8e6, 0.0, &["megabyte_per_second", "МБ/с"]),
            ("gigabyte_per_second", "GB/s", 8e9, 0.0, &["gigabyte_per_second", "ГБ/с"]),
            ("kibibyte_per_second", "KiB/s", 8192.0, 0.0, &["kibibyte_per_second", "КіБ/с"]),
            ("mebibyte_per_second", "MiB/s", 8388608.0, 0.0, &["mebibyte_per_second", "МіБ/с"]),
            ("gibibyte_per_second", "GiB/s", 8589934592.0, 0.0, &["gibibyte_per_second", "ГіБ/с"]),
        ], &["bit_per_second", "kilobit_per_second", "megabit_per_second", "gigabit_per_second"]),
        quantity("data_size", "Data size", &[
            ("byte", "B", 1.0, 0.0, &["byte", "Б"]),
            ("bit", "bit", 0.125, 0.0, &["bit", "біт"]),
            ("kilobit", "kbit", 125.0, 0.0, &["kilobit", "кбіт"]),
            ("megabit", "Mbit", 125000.0, 0.0, &["megabit", "Мбіт"]),
            ("gigabit", "Gbit", 125000000.0, 0.0, &["gigabit", "Гбіт"]),
            ("kilobyte", "kB", 1e3, 0.0, &["kilobyte", "кБ"]),
            ("megabyte", "MB", 1e6, 0.0, &["megabyte", "МБ"]),
            ("gigabyte", "GB", 1e9, 0.0, &["gigabyte", "ГБ"]),
            ("terabyte", "TB", 1e12, 0.0, &["terabyte", "ТБ"]),
            ("petabyte", "PB", 1e15, 0.0, &["petabyte", "ПБ"]),
            ("kibibyte", "KiB", 1024.0, 0.0, &["kibibyte", "КіБ"]),
            ("mebibyte", "MiB", 1048576.0, 0.0, &["mebibyte", "МіБ"]),
            ("gibibyte", "GiB", 1073741824.0, 0.0, &["gibibyte", "ГіБ"]),
            ("tebibyte", "TiB", 1099511627776.0, 0.0, &["tebibyte", "ТіБ"]),
            ("pebibyte", "PiB", 1125899906842624.0, 0.0, &["pebibyte", "ПіБ"]),
        ], &["byte", "kilobyte", "megabyte", "gigabyte", "terabyte", "petabyte"]),
        quantity("precipitation_intensity", "Precipitation intensity", &[
            ("millimeter_per_hour", "mm/h", 1.0, 0.0, &["millimeter_per_hour", "мм/год"]),
            ("millimeter_per_day", "mm/d", 1.0 / 24.0, 0.0, &["millimeter_per_day", "мм/добу"]),
            ("inch_per_hour", "in/h", 25.4, 0.0, &["inch_per_hour", "дюйм/год"]),
            ("inch_per_day", "in/d", 25.4 / 24.0, 0.0, &["inch_per_day", "дюйм/добу"]),
        ], &["millimeter_per_hour"]),
        quantity("volume_flow_rate", "Volume flow rate", &[
            ("cubic_meter_per_hour", "m³/h", 1.0, 0.0, &["cubic_meter_per_hour", "m3/h", "m^3/h", "м³/год"]),
            ("cubic_meter_per_second", "m³/s", 3600.0, 0.0, &["cubic_meter_per_second", "m3/s", "m^3/s", "м³/с"]),
            ("cubic_foot_per_minute", "ft³/min", 1.69901079552, 0.0, &["cubic_foot_per_minute", "ft3/min", "ft^3/min", "фут³/хв"]),
            ("liter_per_hour", "L/h", 0.001, 0.0, &["liter_per_hour", "l/h", "л/год"]),
            ("liter_per_minute", "L/min", 0.06, 0.0, &["liter_per_minute", "l/min", "л/хв"]),
            ("liter_per_second", "L/s", 3.6, 0.0, &["liter_per_second", "l/s", "л/с"]),
            ("gallon_per_minute", "gal/min", 0.22712470704, 0.0, &["gallon_per_minute", "гал/хв"]),
            ("milliliter_per_second", "mL/s", 0.0036, 0.0, &["milliliter_per_second", "ml/s", "мл/с"]),
        ], &["liter_per_hour", "cubic_meter_per_hour"]),
        quantity("conductivity", "Conductivity", &[
            ("microsiemens_per_centimeter", "µS/cm", 1.0, 0.0, &["microsiemens_per_centimeter", "μS/cm", "uS/cm", "мкСм/см"]),
            ("millisiemens_per_centimeter", "mS/cm", 1000.0, 0.0, &["millisiemens_per_centimeter", "мСм/см"]),
            ("siemens_per_centimeter", "S/cm", 1e6, 0.0, &["siemens_per_centimeter", "См/см"]),
        ], &["microsiemens_per_centimeter", "millisiemens_per_centimeter", "siemens_per_centimeter"]),
        quantity("signal_strength", "Signal strength", &[
            ("decibel_milliwatt", "dBm", 1.0, 0.0, &["decibel_milliwatt", "дБм"]),
        ], &["decibel_milliwatt"]),
        quantity("sound_pressure", "Sound pressure", &[
            ("decibel", "dB", 1.0, 0.0, &["decibel", "дБ"]),
            ("a_weighted_decibel", "dBA", 1.0, 0.0, &["a_weighted_decibel", "дБА"]),
        ], &["decibel"]),
        quantity("angle", "Angle", &[
            ("degree", "°", 1.0, 0.0, &["degree", "°"]),
            ("radian", "rad", 180.0 / std::f64::consts::PI, 0.0, &["radian", "рад"]),
        ], &["degree"]),
        quantity("blood_glucose", "Blood glucose", &[
            ("milligram_per_deciliter", "mg/dL", 1.0, 0.0, &["milligram_per_deciliter", "мг/дл"]),
            ("millimole_per_liter", "mmol/L", 18.0, 0.0, &["millimole_per_liter", "ммоль/л"]),
        ], &["milligram_per_deciliter"]),
    ]
}
