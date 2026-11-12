//! Interactive property validation shared by the engine and editor.
use crate::{DataValue, PropertyKind, UiMessage, text};
use std::time::Duration;

/// Parses unsigned property durations, including HH:MM:SS with unbounded hours.
pub fn parse_property_duration(input: &str) -> Option<Duration> {
    let input = input.trim();
    if input.contains(':') {
        let parts: Vec<_> = input.split(':').collect();
        if parts.len() != 3
            || parts[..2]
                .iter()
                .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        let hours: u64 = parts[0].parse().ok()?;
        let minutes: u64 = parts[1].parse().ok()?;
        let seconds: f64 = parts[2].parse().ok()?;
        if minutes >= 60 || !(0.0..60.0).contains(&seconds) {
            return None;
        }
        Duration::from_secs(hours.checked_mul(3600)?.checked_add(minutes * 60)?)
            .checked_add(Duration::try_from_secs_f64(seconds).ok()?)
    } else {
        text::parse_duration(input)
    }
}

/// Validates new typed kinds and normalizes multiselects into definition option order.
/// Legacy Text/Select/Dynamic and hidden snapshots keep their existing contracts.
pub fn normalize_property_value(
    kind: &PropertyKind,
    value: &DataValue,
) -> Result<DataValue, UiMessage> {
    let key = match kind {
        PropertyKind::Duration { .. } => "duration",
        PropertyKind::Time => "time",
        PropertyKind::MultiSelect(_) => "multi-select",
        PropertyKind::Multiline => "text",
        _ => return Ok(value.clone()),
    };
    let error = || {
        UiMessage::new(
            format!("error-property-{key}"),
            format!("Invalid {key} property value"),
        )
    };
    let DataValue::String(input) = value else {
        return Err(error());
    };
    let trimmed = input.trim();
    let valid = match kind {
        PropertyKind::Duration { signed } => {
            let magnitude = if *signed {
                trimmed
                    .strip_prefix('-')
                    .or_else(|| trimmed.strip_prefix('+'))
                    .unwrap_or(trimmed)
            } else {
                trimmed
            };
            trimmed.is_empty()
                || (!magnitude.starts_with(['-', '+'])
                    && parse_property_duration(magnitude).is_some())
        }
        PropertyKind::Time => trimmed.is_empty() || text::parse_time(trimmed).is_some(),
        PropertyKind::MultiSelect(options) => {
            let selected: Vec<_> = trimmed
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            if selected
                .iter()
                .any(|s| !options.iter().any(|(v, _)| v == s))
            {
                return Err(error());
            }
            return Ok(DataValue::String(
                options
                    .iter()
                    .filter(|(v, _)| selected.contains(&v.as_str()))
                    .map(|(v, _)| v.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            ));
        }
        PropertyKind::Multiline => true,
        _ => unreachable!(),
    };
    if valid {
        Ok(value.clone())
    } else {
        Err(error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_values_are_validated_and_multiselects_are_canonical() {
        for signed in [false, true] {
            let kind = PropertyKind::Duration { signed };
            for text in ["", "90", "1h 30m", "500ms", "28:30:15", "00:00:00.125"] {
                assert!(
                    normalize_property_value(&kind, &text.into()).is_ok(),
                    "{text}"
                );
            }
            for text in ["abc", "10t", "01:60:00", "--5", "+-5", "NaN", "inf"] {
                assert!(
                    normalize_property_value(&kind, &text.into()).is_err(),
                    "{text}"
                );
            }
            assert_eq!(
                normalize_property_value(&kind, &"-01:30:00".into()).is_ok(),
                signed
            );
            assert!(normalize_property_value(&kind, &true.into()).is_err());
        }
        for text in ["", "07:30", "23:59:59"] {
            assert!(normalize_property_value(&PropertyKind::Time, &text.into()).is_ok());
        }
        for text in ["tomorrow", "24:00", "12:60", "12:30:99"] {
            assert!(
                normalize_property_value(&PropertyKind::Time, &text.into()).is_err(),
                "{text}"
            );
        }
        let options = PropertyDefinition::multi_select(
            "days",
            "Days",
            &[("mon", "Monday"), ("tue", "Tuesday")],
        )
        .kind;
        assert_eq!(
            normalize_property_value(&options, &"tue,mon,tue".into()).unwrap(),
            DataValue::from("mon,tue")
        );
        assert_eq!(
            normalize_property_value(&options, &"".into()).unwrap(),
            DataValue::from("")
        );
        assert!(normalize_property_value(&options, &"wed".into()).is_err());
        assert_eq!(
            normalize_property_value(&PropertyKind::Multiline, &" a\nb ".into()).unwrap(),
            DataValue::from(" a\nb ")
        );
        assert!(normalize_property_value(&PropertyKind::Multiline, &false.into()).is_err());
        assert!(normalize_property_value(&PropertyKind::Hidden, &true.into()).is_ok());
    }
    use crate::PropertyDefinition;

    #[test]
    fn duration_clock_checks_overflow_and_ranges() {
        assert_eq!(
            parse_property_duration("27:30:01"),
            Some(Duration::from_secs(99001))
        );
        assert!(parse_property_duration("18446744073709551615:00:00").is_none());
        assert!(parse_property_duration("-01:00:00").is_none());
    }
}
