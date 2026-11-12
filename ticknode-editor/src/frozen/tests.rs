use super::*;
use crate::i18n::{Locale, with_locale};

fn pin(direction: PinDirection, kind: PinType) -> PinSignature {
    PinSignature {
        id: PinId(1),
        direction,
        kind,
        required: false,
        name: "Value".into(),
    }
}

#[test]
fn contract_changes_describe_direction_kind_and_required_inputs_in_both_locales() {
    let old = pin(PinDirection::Input, PinType::Exec);
    let new = pin(PinDirection::Output, PinType::Data(DataType::Number));
    let removed = PinChange::Removed { pin: old.clone() };
    let changed = PinChange::Changed {
        old,
        new: new.clone(),
    };
    let required = PinChange::RequiredAdded {
        pin: PinSignature {
            direction: PinDirection::Input,
            required: true,
            ..new
        },
    };
    let clean = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
    for (locale, removed_text, changed_text, required_text) in [
        (
            Locale::En,
            "Removed input pin “Value” (Execution)",
            "Pin “Value”: input · Execution → output · Number",
            "New required input “Value” (Number)",
        ),
        (
            Locale::Uk,
            "Видалено вхідний пін «Value» (Виконання)",
            "Пін «Value»: вхідний · Виконання → вихідний · Число",
            "Новий обов’язковий вхід «Value» (Число)",
        ),
    ] {
        with_locale(locale, || {
            let format = |change| {
                clean(format_change("example", change, |ty| {
                    i18n::data_type_title(&TypeRegistry::new(), ty)
                }))
            };
            assert_eq!(format(&removed), removed_text);
            assert_eq!(format(&changed), changed_text);
            assert_eq!(format(&required), required_text);
        });
    }
}

#[test]
fn changed_data_types_use_the_existing_type_title_formatter() {
    with_locale(Locale::En, || {
        let change = PinChange::Changed {
            old: pin(PinDirection::Input, PinType::Data(DataType::String)),
            new: pin(
                PinDirection::Input,
                PinType::Data(DataType::Named("example.state".into())),
            ),
        };
        let text = format_change("example", &change, |ty| match ty {
            DataType::Named(_) => "Custom state".into(),
            _ => i18n::data_type_title(&TypeRegistry::new(), ty),
        });
        assert!(text.contains("Text"));
        assert!(text.contains("Custom state"));
    });
}
