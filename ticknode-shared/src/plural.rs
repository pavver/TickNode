use intl_pluralrules::operands::PluralOperands;
use intl_pluralrules::{PluralCategory as IprCategory, PluralRuleType, PluralRules};
use unic_langid::LanguageIdentifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralCategory {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Two => "two",
            Self::Few => "few",
            Self::Many => "many",
            Self::Other => "other",
        }
    }
}

impl From<IprCategory> for PluralCategory {
    fn from(cat: IprCategory) -> Self {
        match cat {
            IprCategory::ZERO => Self::Zero,
            IprCategory::ONE => Self::One,
            IprCategory::TWO => Self::Two,
            IprCategory::FEW => Self::Few,
            IprCategory::MANY => Self::Many,
            IprCategory::OTHER => Self::Other,
        }
    }
}

fn format_number(n: f64) -> String {
    let s = format!("{:.3}", n);
    if let Some((int_part, frac_part)) = s.split_once('.') {
        let trimmed_frac = frac_part.trim_end_matches('0');
        if trimmed_frac.is_empty() {
            int_part.to_string()
        } else {
            format!("{int_part}.{trimmed_frac}")
        }
    } else {
        s
    }
}

fn get_rules(locale: &str) -> Option<PluralRules> {
    if let Ok(langid) = locale.parse::<LanguageIdentifier>()
        && let Ok(rules) = PluralRules::create(langid, PluralRuleType::CARDINAL)
    {
        return Some(rules);
    }
    let en_langid: LanguageIdentifier = "en".parse().ok()?;
    PluralRules::create(en_langid, PluralRuleType::CARDINAL).ok()
}

pub fn category(locale: &str, n: f64) -> PluralCategory {
    let rules = match get_rules(locale) {
        Some(r) => r,
        None => return PluralCategory::Other,
    };
    let formatted = format_number(n);
    let operands = match PluralOperands::try_from(formatted.as_str()) {
        Ok(op) => op,
        Err(_) => return PluralCategory::Other,
    };
    rules
        .select(operands)
        .map(PluralCategory::from)
        .unwrap_or(PluralCategory::Other)
}

pub fn integer_categories(locale: &str) -> Vec<PluralCategory> {
    let rules = match get_rules(locale) {
        Some(r) => r,
        None => return vec![PluralCategory::Other],
    };
    let mut seen = [false; 6];
    for i in 0usize..=200 {
        let op = PluralOperands::from(i);
        if let Ok(cat) = rules.select(op) {
            let our_cat = PluralCategory::from(cat);
            seen[our_cat as usize] = true;
        }
    }
    const CLDR_ORDER: [PluralCategory; 6] = [
        PluralCategory::Zero,
        PluralCategory::One,
        PluralCategory::Two,
        PluralCategory::Few,
        PluralCategory::Many,
        PluralCategory::Other,
    ];
    CLDR_ORDER
        .iter()
        .copied()
        .filter(|&c| seen[c as usize])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plural_categories() {
        assert_eq!(category("en", 1.0), PluralCategory::One);
        assert_eq!(category("en", -1.0), PluralCategory::One);
        assert_eq!(category("en", 2.0), PluralCategory::Other);
        assert_eq!(category("en", 1.5), PluralCategory::Other);

        assert_eq!(category("uk", 1.0), PluralCategory::One);
        assert_eq!(category("uk", 2.0), PluralCategory::Few);
        assert_eq!(category("uk", 5.0), PluralCategory::Many);
        assert_eq!(category("uk", 11.0), PluralCategory::Many);
        assert_eq!(category("uk", 21.0), PluralCategory::One);
        assert_eq!(category("uk", 1.5), PluralCategory::Other);

        assert_eq!(
            integer_categories("en"),
            vec![PluralCategory::One, PluralCategory::Other]
        );
        assert_eq!(
            integer_categories("uk"),
            vec![
                PluralCategory::One,
                PluralCategory::Few,
                PluralCategory::Many
            ]
        );
    }
}
