//! Built-in physical quantity definitions. Values use the first (canonical) unit.
use super::{QuantityKind, UnitDefinition};
use crate::{TypeDefinition, TypeKind};

#[rustfmt::skip]
pub fn builtin_quantities() -> Vec<TypeDefinition> {
    let mut definitions = vec![
        quantity("power", "Power", &[
            ("watt", "W", 1.0, 0.0, &["watt", "Вт"]),
            ("kilowatt", "kW", 1000.0, 0.0, &["kilowatt", "кВт"]),
            ("megawatt", "MW", 1000000.0, 0.0, &["megawatt", "МВт"]),
            ("milliwatt", "mW", 0.001, 0.0, &["milliwatt", "мВт"]),
            ("gigawatt", "GW", 1e9, 0.0, &["gigawatt", "ГВт"]),
            ("terawatt", "TW", 1e12, 0.0, &["terawatt", "ТВт"]),
            ("btu_per_hour", "BTU/h", 0.29307107, 0.0, &["btu_per_hour", "БТО/год"]),
        ], &["watt", "kilowatt", "megawatt", "gigawatt", "terawatt"]),
        quantity("energy", "Energy", &[
            ("watt_hour", "Wh", 1.0, 0.0, &["watt_hour", "Вт·год"]),
            ("kilowatt_hour", "kWh", 1000.0, 0.0, &["kilowatt_hour", "кВт·год"]),
            ("megawatt_hour", "MWh", 1000000.0, 0.0, &["megawatt_hour", "МВт·год"]),
            ("joule", "J", 1.0 / 3600.0, 0.0, &["joule", "Дж"]),
            ("kilojoule", "kJ", 1.0 / 3.6, 0.0, &["kilojoule", "кДж"]),
            ("megajoule", "MJ", 1000.0 / 3.6, 0.0, &["megajoule", "МДж"]),
            ("gigawatt_hour", "GWh", 1e9, 0.0, &["gigawatt_hour", "ГВт·год"]),
            ("terawatt_hour", "TWh", 1e12, 0.0, &["terawatt_hour", "ТВт·год"]),
            ("milliwatt_hour", "mWh", 0.001, 0.0, &["milliwatt_hour", "мВт·год"]),
            ("gigajoule", "GJ", 1e9 / 3600.0, 0.0, &["gigajoule", "ГДж"]),
            ("calorie", "cal", 4.184 / 3600.0, 0.0, &["calorie", "кал"]),
            ("kilocalorie", "kcal", 4184.0 / 3600.0, 0.0, &["kilocalorie", "ккал"]),
            ("megacalorie", "Mcal", 4.184e6 / 3600.0, 0.0, &["megacalorie", "Мкал"]),
            ("gigacalorie", "Gcal", 4.184e9 / 3600.0, 0.0, &["gigacalorie", "Гкал"]),
        ], &["watt_hour", "kilowatt_hour", "megawatt_hour", "gigawatt_hour", "terawatt_hour"]),
        quantity("voltage", "Voltage", &[
            ("volt", "V", 1.0, 0.0, &["volt", "В"]),
            ("millivolt", "mV", 0.001, 0.0, &["millivolt", "мВ"]),
            ("kilovolt", "kV", 1000.0, 0.0, &["kilovolt", "кВ"]),
            ("microvolt", "µV", 1e-6, 0.0, &["microvolt", "uV", "μV", "мкВ"]),
            ("megavolt", "MV", 1e6, 0.0, &["megavolt", "МВ"]),
        ], &["microvolt", "millivolt", "volt", "kilovolt", "megavolt"]),
        quantity("current", "Current", &[
            ("ampere", "A", 1.0, 0.0, &["ampere", "А"]),
            ("milliampere", "mA", 0.001, 0.0, &["milliampere", "мА"]),
        ], &["milliampere", "ampere"]),
        quantity("apparent_power", "Apparent power", &[
            ("volt_ampere", "VA", 1.0, 0.0, &["volt_ampere", "ВА", "В·А"]),
            ("kilovolt_ampere", "kVA", 1000.0, 0.0, &["kilovolt_ampere", "кВА", "кВ·А"]),
            ("millivolt_ampere", "mVA", 0.001, 0.0, &["millivolt_ampere", "мВ·А"]),
        ], &["millivolt_ampere", "volt_ampere", "kilovolt_ampere"]),
        quantity("frequency", "Frequency", &[
            ("hertz", "Hz", 1.0, 0.0, &["hertz", "Гц"]),
            ("kilohertz", "kHz", 1000.0, 0.0, &["kilohertz", "кГц"]),
            ("megahertz", "MHz", 1000000.0, 0.0, &["megahertz", "МГц"]),
            ("gigahertz", "GHz", 1000000000.0, 0.0, &["gigahertz", "ГГц"]),
        ], &["hertz", "kilohertz", "megahertz", "gigahertz"]),
        quantity("temperature", "Temperature", &[
            ("celsius", "°C", 1.0, 0.0, &["celsius", "degC", "C", "°С"]),
            ("fahrenheit", "°F", 5.0 / 9.0, -160.0 / 9.0, &["fahrenheit", "degF", "F"]),
            ("kelvin", "K", 1.0, -273.15, &["kelvin", "К"]),
        ], &["celsius"]),
        quantity("percentage", "Percentage", &[
            ("percent", "%", 1.0, 0.0, &["percent"]),
        ], &["percent"]),
        quantity("pressure", "Pressure", &[
            ("pascal", "Pa", 1.0, 0.0, &["pascal", "Па"]),
            ("hectopascal", "hPa", 100.0, 0.0, &["hectopascal", "гПа"]),
            ("kilopascal", "kPa", 1000.0, 0.0, &["kilopascal", "кПа"]),
            ("millibar", "mbar", 100.0, 0.0, &["millibar", "мбар"]),
            ("bar", "bar", 100000.0, 0.0, &["bar", "бар"]),
            ("psi", "psi", 6894.757, 0.0, &["psi"]),
            ("mmhg", "mmHg", 133.322, 0.0, &["mmhg", "мм рт. ст."]),
            ("inhg", "inHg", 3386.389, 0.0, &["inhg", "дюйм рт. ст."]),
            ("centibar", "cbar", 1000.0, 0.0, &["centibar", "сбар"]),
            ("inch_of_water", "inH2O", 249.08891, 0.0, &["inch_of_water", "дюйм вод. ст."]),
        ], &["hectopascal"]),
        quantity("speed", "Speed", &[
            ("meter_per_second", "m/s", 1.0, 0.0, &["meter_per_second", "м/с"]),
            ("kilometer_per_hour", "km/h", 1.0 / 3.6, 0.0, &["kilometer_per_hour", "kmh", "км/год"]),
            ("mile_per_hour", "mph", 0.44704, 0.0, &["mile_per_hour", "миль/год"]),
            ("knot", "kn", 0.514444, 0.0, &["knot", "вуз"]),
            ("foot_per_second", "ft/s", 0.3048, 0.0, &["foot_per_second", "фут/с"]),
            ("inch_per_day", "in/d", 0.0254 / 86400.0, 0.0, &["inch_per_day", "дюйм/добу"]),
            ("inch_per_hour", "in/h", 0.0254 / 3600.0, 0.0, &["inch_per_hour", "дюйм/год"]),
            ("inch_per_second", "in/s", 0.0254, 0.0, &["inch_per_second", "дюйм/с"]),
            ("millimeter_per_day", "mm/d", 0.001 / 86400.0, 0.0, &["millimeter_per_day", "мм/добу"]),
            ("millimeter_per_second", "mm/s", 0.001, 0.0, &["millimeter_per_second", "мм/с"]),
        ], &["meter_per_second"]),
        quantity("length", "Length", &[
            ("meter", "m", 1.0, 0.0, &["meter", "м"]),
            ("millimeter", "mm", 0.001, 0.0, &["millimeter", "мм"]),
            ("centimeter", "cm", 0.01, 0.0, &["centimeter", "см"]),
            ("kilometer", "km", 1000.0, 0.0, &["kilometer", "км"]),
            ("inch", "in", 0.0254, 0.0, &["inch", "дюйм"]),
            ("foot", "ft", 0.3048, 0.0, &["foot", "фут"]),
            ("yard", "yd", 0.9144, 0.0, &["yard", "ярд"]),
            ("mile", "mi", 1609.344, 0.0, &["mile", "миля"]),
            ("nautical_mile", "nmi", 1852.0, 0.0, &["nautical_mile", "мор. миля"]),
        ], &["millimeter", "meter", "kilometer"]),
        quantity("volume", "Volume", &[
            ("liter", "L", 1.0, 0.0, &["liter", "l", "л"]),
            ("milliliter", "mL", 0.001, 0.0, &["milliliter", "ml", "мл"]),
            ("cubic_meter", "m³", 1000.0, 0.0, &["cubic_meter", "m3", "м³"]),
            ("gallon", "gal", 3.785411784, 0.0, &["gallon", "гал"]),
            ("cubic_foot", "ft³", 28.316846592, 0.0, &["cubic_foot", "ft3", "фут³"]),
            ("fluid_ounce", "fl. oz.", 0.0295735295625, 0.0, &["fluid_ounce", "рід. унц."]),
            ("ccf", "CCF", 2831.6846592, 0.0, &["ccf", "сотня фут³"]),
            ("mcf", "MCF", 28316.846592, 0.0, &["mcf", "тисяча фут³"]),
        ], &["milliliter", "liter", "cubic_meter"]),
        quantity("mass", "Mass", &[
            ("kilogram", "kg", 1.0, 0.0, &["kilogram", "кг"]),
            ("gram", "g", 0.001, 0.0, &["gram", "г"]),
            ("milligram", "mg", 1e-6, 0.0, &["milligram", "мг"]),
            ("tonne", "t", 1000.0, 0.0, &["tonne", "т"]),
            ("pound", "lb", 0.45359237, 0.0, &["pound", "фунт"]),
            ("ounce", "oz", 0.028349523125, 0.0, &["ounce", "унц"]),
            ("microgram", "µg", 1e-9, 0.0, &["microgram", "ug", "μg", "мкг"]),
            ("stone", "st", 6.35029318, 0.0, &["stone", "стоун"]),
        ], &["milligram", "gram", "kilogram", "tonne"]),
        quantity("illuminance", "Illuminance", &[
            ("lux", "lx", 1.0, 0.0, &["lux", "лк"]),
        ], &["lux"]),
        quantity("irradiance", "Irradiance", &[
            ("watt_per_square_meter", "W/m²", 1.0, 0.0, &["watt_per_square_meter", "W/m2", "Вт/м²"]),
            ("btu_per_hour_square_foot", "BTU/(h⋅ft²)", 3.15459075, 0.0, &["btu_per_hour_square_foot", "BTU/(h*ft2)", "BTU/(h*ft^2)", "БТО/(год·фут²)"]),
        ], &["watt_per_square_meter"]),
        quantity("concentration", "Concentration", &[
            ("microgram_per_cubic_meter", "µg/m³", 1.0, 0.0, &["microgram_per_cubic_meter", "ug/m3", "μg/m³", "мкг/м³"]),
            ("milligram_per_cubic_meter", "mg/m³", 1000.0, 0.0, &["milligram_per_cubic_meter", "mg/m3", "мг/м³"]),
            ("gram_per_cubic_meter", "g/m³", 1e6, 0.0, &["gram_per_cubic_meter", "g/m3", "g/m^3", "г/м³"]),
        ], &["microgram_per_cubic_meter", "milligram_per_cubic_meter"]),
        quantity("parts", "Parts", &[
            ("ppm", "ppm", 1.0, 0.0, &["ppm", "млн⁻¹"]),
            ("ppb", "ppb", 0.001, 0.0, &["ppb"]),
        ], &["ppm"]),
    ];
    definitions.extend(super::catalog_extra::quantities());
    definitions.extend(super::indices::quantities());
    definitions.extend(super::currencies::quantities());
    definitions
}

type UnitSpec<'a> = (&'a str, &'a str, f64, f64, &'a [&'a str]);
pub(super) fn quantity(
    name: &str,
    title: &str,
    units: &[UnitSpec<'_>],
    auto: &[&str],
) -> TypeDefinition {
    TypeDefinition {
        name: format!("quantity.{name}"),
        title: title.into(),
        description: format!("{title} in physical units"),
        color: Some("#d9a441".into()),
        kind: TypeKind::Quantity(QuantityKind {
            canonical: units[0].0.into(),
            units: units
                .iter()
                .map(|(id, symbol, factor, offset, aliases)| UnitDefinition {
                    id: (*id).into(),
                    symbol: (*symbol).into(),
                    factor: *factor,
                    offset: *offset,
                    aliases: aliases.iter().map(|a| (*a).into()).collect(),
                })
                .collect(),
            auto: auto.iter().map(|id| (*id).into()).collect(),
            decimals: None,
        }),
    }
}

pub(super) fn with_decimals(mut definition: TypeDefinition, decimals: u8) -> TypeDefinition {
    if let TypeKind::Quantity(kind) = &mut definition.kind {
        kind.decimals = Some(decimals);
    }
    definition
}
