use super::*;
use crate::{*, condition::{ops_in, CompareOp}};
fn registry() -> TypeRegistry { TypeRegistry::from_definitions(builtin_quantities()) }
fn named(s: &str) -> DataType { DataType::Named(format!("quantity.{s}")) }

#[test]
fn catalog_is_consistent() {
    assert_eq!(catalog().len(), 17);
    for def in catalog() {
        let q = def.quantity().unwrap();
        let canonical = q.units.iter().find(|u| u.id == q.canonical).unwrap();
        assert_eq!((canonical.factor, canonical.offset), (1.0, 0.0));
        let mut ids = std::collections::HashSet::new();
        let mut aliases = std::collections::HashMap::new();
        for u in &q.units {
            assert!(ids.insert(&u.id));
            assert!(u.factor.is_finite() && u.factor > 0.0 && u.offset.is_finite());
            for alias in std::iter::once(&u.symbol).chain(&u.aliases) {
                if let Some(old) = aliases.insert(alias, &u.id) { assert_eq!(old, &u.id); }
            }
        }
        let factors: Vec<_> = q.auto.iter().map(|id| q.units.iter().find(|u| &u.id == id).unwrap().factor).collect();
        assert!(!factors.is_empty() && factors.windows(2).all(|w| w[0] < w[1]));
    }
}
#[test]
fn parsing_and_public_conversion_api() {
    let r = registry();
    for (s, expected) in [("2 kW",2000.0), ("2,5 кВт",2500.0), ("1 mW",0.001), ("1 MW",1e6), ("230",230.0), ("-2kW",-2000.0)] {
        assert_eq!(text::parse_value_in(&r,s,&named("power")), Ok(DataValue::from(expected)));
    }
    for s in ["5 V", "1 mw", "NaN", "1e309 W"] { assert!(text::parse_value_in(&r,s,&named("power")).is_err(), "{s}"); }
    let temp = parse_quantity("-3 °F", kind("quantity.temperature").unwrap()).unwrap();
    assert!((temp - (-35.0 * 5.0 / 9.0)).abs() < 1e-10);
    let (name,u) = unit_by_symbol("кВт").unwrap();
    assert_eq!(name,"quantity.power");
    assert_eq!(from_canonical(to_canonical(2.5,u),u),2.5);
    assert_eq!(text::format_value(&DataValue::from(2000)),"2000");
}
struct Ukrainian;
impl QuantityLabels for Ukrainian {
    fn unit_symbol(&self,id:&str)->Option<String> { match id { "watt"=>Some("Вт".into()), "kilowatt"=>Some("кВт".into()), _=>None } }
    fn decimal_separator(&self)->String { ",".into() }
}
#[test]
fn formatting_and_rounding() {
    let q=kind("quantity.power").unwrap(); let f=QuantityFormat::default();
    for (v,s) in [(1970.0,"1.97 kW"),(57.0,"57 W"),(57.02,"57 W"),(57.3,"57.3 W"),(999.7,"1 kW"),(0.0,"0 W"),(-999.7,"-1 kW"),(1500.0,"1.5 kW")] {
        assert_eq!(format_quantity(v,q,&f,&QuantityDisplay::default_for(q),&CanonicalLabels),s);
    }
    assert_eq!(format_quantity(1970.0,q,&f,&QuantityDisplay::default_for(q),&Ukrainian),"1,97 кВт");
    assert_eq!(format_quantity(57.0,q,&f,&QuantityDisplay::default_for(q),&Ukrainian),"57 Вт");
    assert_eq!(format_quantity(12345.0,q,&QuantityFormat{unit:Some("watt".into()),digits:3},&QuantityDisplay::default_for(q),&CanonicalLabels),"12345 W");
    assert_eq!(format_quantity(0.0,kind("quantity.temperature").unwrap(),&QuantityFormat{unit:Some("fahrenheit".into()),digits:3},&QuantityDisplay::default_for(kind("quantity.temperature").unwrap()),&CanonicalLabels),"32 °F");
    assert_eq!(format_quantity(1970.0,q,&QuantityFormat{unit:None,digits:2},&QuantityDisplay::default_for(q),&CanonicalLabels),"2 kW");
}
#[test]
fn quantity_type_rules_and_validation() {
    let r=registry(); let p=named("power"); let e=named("energy");
    assert!(r.compatible(&p,&p)); assert!(r.compatible(&p,&DataType::Number));
    assert!(!r.compatible(&DataType::Number,&p));
    assert!(r.convertible(&DataType::Number,&p).is_some());
    assert!(r.convertible(&DataType::String,&p).is_some());
    assert!(r.convertible(&p,&DataType::String).is_some());
    assert!(!r.compatible(&p,&e)); assert!(r.convertible(&p,&e).is_none());
    assert!(input_value_editable(&r,&p)); assert!(input_value_fits(&r,&p,&DataValue::from(2)));
    assert!(!input_value_fits(&r,&p,&DataValue::from("2 W")));
    assert!(ops_in(&r,&p).contains(&CompareOp::Gt));
    let mut r=TypeRegistry::new();
    assert!(r.insert_all(&[TypeDefinition::new_enum("quantity.fake","Fake",&[("x","X")])]).is_err());
}
