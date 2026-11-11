//! Independent currencies; the catalog never supplies exchange rates.

use super::catalog::{quantity, with_decimals};
use crate::TypeDefinition;

#[rustfmt::skip]
pub(super) fn quantities() -> Vec<TypeDefinition> {
    vec![
        currency("UAH", "Ukrainian hryvnia", &["грн", "₴"], 2),
        currency("USD", "US dollar", &[], 2),
        currency("EUR", "Euro", &["€", "євро"], 2),
        currency("GBP", "Pound sterling", &["£"], 2),
        currency("PLN", "Polish zloty", &["zł"], 2),
        currency("CHF", "Swiss franc", &[], 2),
        currency("CZK", "Czech koruna", &[], 2),
        currency("HUF", "Hungarian forint", &[], 0),
        currency("RON", "Romanian leu", &[], 2),
        currency("BGN", "Bulgarian lev", &[], 2),
        currency("SEK", "Swedish krona", &[], 2),
        currency("NOK", "Norwegian krone", &[], 2),
        currency("DKK", "Danish krone", &[], 2),
        currency("ISK", "Icelandic krona", &[], 2),
        currency("TRY", "Turkish lira", &["₺"], 2),
        currency("GEL", "Georgian lari", &["₾"], 2),
        currency("MDL", "Moldovan leu", &[], 2),
        currency("KZT", "Kazakhstani tenge", &["₸"], 2),
        currency("ILS", "Israeli new shekel", &["₪"], 2),
        currency("JPY", "Japanese yen", &[], 0),
        currency("CNY", "Chinese yuan", &[], 2),
        currency("KRW", "South Korean won", &["₩"], 0),
        currency("INR", "Indian rupee", &["₹"], 2),
        currency("CAD", "Canadian dollar", &[], 2),
        currency("AUD", "Australian dollar", &[], 2),
        currency("NZD", "New Zealand dollar", &[], 2),
        currency("SGD", "Singapore dollar", &[], 2),
        currency("HKD", "Hong Kong dollar", &[], 2),
        currency("BRL", "Brazilian real", &["R$"], 2),
        currency("MXN", "Mexican peso", &[], 2),
        currency("ZAR", "South African rand", &[], 2),
        currency("AED", "UAE dirham", &[], 2),
    ]
}

fn currency(code: &str, title: &str, aliases: &[&str], decimals: u8) -> TypeDefinition {
    let id = code.to_ascii_lowercase();
    let aliases: Vec<_> = std::iter::once(code)
        .chain(aliases.iter().copied())
        .collect();
    let mut definition = with_decimals(
        quantity(
            &format!("currency_{id}"),
            title,
            &[(id.as_str(), code, 1.0, 0.0, &aliases)],
            &[id.as_str()],
        ),
        decimals,
    );
    definition.description = format!("Money in {code}; currencies never convert");
    definition.color = Some("#5aa469".into());
    definition
}
