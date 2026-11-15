use super::super::*;
use crate::DataType;
use std::collections::HashMap;

fn infer(source: &str, vars: &[(&str, DataType)]) -> Result<DataType, FormulaError> {
    let compiled = compile(source)?;
    let map: HashMap<String, DataType> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    compiled.infer_type(&map)
}

#[test]
fn test_quantity_inference_basic() {
    let power = DataType::Named("quantity.power".into());
    let energy = DataType::Named("quantity.energy".into());

    // Same quantity keeps that quantity
    assert_eq!(
        infer("a + b", &[("a", power.clone()), ("b", power.clone())]).unwrap(),
        power
    );
    assert_eq!(
        infer("a - b", &[("a", power.clone()), ("b", power.clone())]).unwrap(),
        power
    );
    assert_eq!(
        infer("min(a, b)", &[("a", power.clone()), ("b", power.clone())]).unwrap(),
        power
    );
    assert_eq!(
        infer("max(a, b)", &[("a", power.clone()), ("b", power.clone())]).unwrap(),
        power
    );
    assert_eq!(
        infer("clamp(a, 0, 1000)", &[("a", power.clone())]).unwrap(),
        power
    );
    assert_eq!(
        infer("avg(a, b)", &[("a", power.clone()), ("b", power.clone())]).unwrap(),
        power
    );
    assert_eq!(infer("round(a)", &[("a", power.clone())]).unwrap(), power);
    assert_eq!(infer("abs(a)", &[("a", power.clone())]).unwrap(), power);

    // Mixing quantity with literal or plain number in + / - treats number as canonical units
    assert_eq!(infer("a + 100", &[("a", power.clone())]).unwrap(), power);
    assert_eq!(infer("100 + a", &[("a", power.clone())]).unwrap(), power);
    assert_eq!(infer("a - 50", &[("a", power.clone())]).unwrap(), power);

    // Combining different quantities in + / - / compare is an error
    assert!(infer("a + b", &[("a", power.clone()), ("b", energy.clone())]).is_err());
    assert!(infer("a - b", &[("a", power.clone()), ("b", energy.clone())]).is_err());
    assert!(infer("a > b", &[("a", power.clone()), ("b", energy.clone())]).is_err());
    assert!(infer("min(a, b)", &[("a", power.clone()), ("b", energy.clone())]).is_err());
}

#[test]
fn test_quantity_multiplication_and_division() {
    let power = DataType::Named("quantity.power".into());
    let energy = DataType::Named("quantity.energy".into());

    // Quantity * Number = Quantity
    assert_eq!(infer("p * 2", &[("p", power.clone())]).unwrap(), power);
    assert_eq!(infer("2 * p", &[("p", power.clone())]).unwrap(), power);

    // Quantity / Number = Quantity
    assert_eq!(infer("p / 2", &[("p", power.clone())]).unwrap(), power);

    // Quantity / same Quantity = Number
    assert_eq!(
        infer("p1 / p2", &[("p1", power.clone()), ("p2", power.clone())]).unwrap(),
        DataType::Number
    );

    // Other products / ratios have unknown dimension -> Number
    assert_eq!(
        infer("p * eng", &[("p", power.clone()), ("eng", energy.clone())]).unwrap(),
        DataType::Number
    );
    assert_eq!(
        infer("p / eng", &[("p", power.clone()), ("eng", energy.clone())]).unwrap(),
        DataType::Number
    );
}

#[test]
fn test_comparisons_and_logic_yield_bool() {
    let power = DataType::Named("quantity.power".into());

    assert_eq!(
        infer("p > 100", &[("p", power.clone())]).unwrap(),
        DataType::Bool
    );
    assert_eq!(
        infer("p1 == p2", &[("p1", power.clone()), ("p2", power.clone())]).unwrap(),
        DataType::Bool
    );
    assert_eq!(
        infer("between(p, 10, 50)", &[("p", power.clone())]).unwrap(),
        DataType::Bool
    );
    assert_eq!(
        infer(
            "p > 100 && flag",
            &[("p", power.clone()), ("flag", DataType::Bool)]
        )
        .unwrap(),
        DataType::Bool
    );
    assert_eq!(
        infer("!flag", &[("flag", DataType::Bool)]).unwrap(),
        DataType::Bool
    );
}

#[test]
fn test_reconcile_branches_and_conditionals() {
    let power = DataType::Named("quantity.power".into());
    let energy = DataType::Named("quantity.energy".into());

    assert_eq!(
        infer(
            "flag ? p1 : p2",
            &[
                ("flag", DataType::Bool),
                ("p1", power.clone()),
                ("p2", power.clone()),
            ]
        )
        .unwrap(),
        power
    );
    assert_eq!(
        infer(
            "flag ? p1 : 10",
            &[("flag", DataType::Bool), ("p1", power.clone())]
        )
        .unwrap(),
        power
    );
    assert_eq!(
        infer(
            "flag ? 10 : p1",
            &[("flag", DataType::Bool), ("p1", power.clone())]
        )
        .unwrap(),
        power
    );
    assert_eq!(
        infer("flag ? true : false", &[("flag", DataType::Bool)]).unwrap(),
        DataType::Bool
    );
    assert_eq!(
        infer("flag ? 10 : 20", &[("flag", DataType::Bool)]).unwrap(),
        DataType::Number
    );

    assert_eq!(
        infer(
            "if(flag, p1, p2)",
            &[
                ("flag", DataType::Bool),
                ("p1", power.clone()),
                ("p2", power.clone()),
            ]
        )
        .unwrap(),
        power
    );

    assert!(
        infer(
            "flag ? p1 : en",
            &[
                ("flag", DataType::Bool),
                ("p1", power.clone()),
                ("en", energy.clone()),
            ]
        )
        .is_err()
    );

    assert!(
        infer(
            "if(flag, p1, en)",
            &[
                ("flag", DataType::Bool),
                ("p1", power.clone()),
                ("en", energy.clone()),
            ]
        )
        .is_err()
    );
}

#[test]
fn test_map_and_clamp_incompatible_quantities() {
    let power = DataType::Named("quantity.power".into());
    let energy = DataType::Named("quantity.energy".into());

    assert_eq!(
        infer(
            "map(p, 0, 100, p1, p2)",
            &[
                ("p", DataType::Number),
                ("p1", power.clone()),
                ("p2", power.clone()),
            ]
        )
        .unwrap(),
        power
    );
    assert_eq!(
        infer(
            "map(p, 0, 100, 0, p2)",
            &[("p", DataType::Number), ("p2", power.clone())]
        )
        .unwrap(),
        power
    );
    assert_eq!(
        infer(
            "map(p, 0, 100, p1, 100)",
            &[("p", DataType::Number), ("p1", power.clone())]
        )
        .unwrap(),
        power
    );
    assert_eq!(
        infer("map(p, 0, 100, 0, 100)", &[("p", DataType::Number)]).unwrap(),
        DataType::Number
    );

    assert!(
        infer(
            "clamp(p, p1, en)",
            &[
                ("p", power.clone()),
                ("p1", power.clone()),
                ("en", energy.clone()),
            ]
        )
        .is_err()
    );
}
