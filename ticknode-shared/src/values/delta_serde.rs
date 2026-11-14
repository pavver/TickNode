use chrono::TimeDelta;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Duration")]
pub struct DeltaParts {
    pub secs: i64,
    pub nanos: u32,
}

/// Splits a `TimeDelta` into floored seconds and non-negative nanoseconds in `0..1_000_000_000`.
pub fn split_delta(delta: TimeDelta) -> (i64, u32) {
    let num_secs = delta.num_seconds();
    let subsec_nanos = delta.subsec_nanos();
    if subsec_nanos < 0 {
        (
            num_secs.saturating_sub(1),
            (subsec_nanos + 1_000_000_000) as u32,
        )
    } else {
        (num_secs, subsec_nanos as u32)
    }
}

/// Recombines floored seconds and non-negative nanoseconds into a `TimeDelta`.
pub fn combine_delta(secs: i64, nanos: u32) -> Option<TimeDelta> {
    TimeDelta::new(secs, nanos)
}

pub fn serialize<S>(delta: &TimeDelta, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let (secs, nanos) = split_delta(*delta);
    DeltaParts { secs, nanos }.serialize(serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<TimeDelta, D::Error>
where
    D: Deserializer<'de>,
{
    let parts = DeltaParts::deserialize(deserializer)?;
    combine_delta(parts.secs, parts.nanos)
        .ok_or_else(|| serde::de::Error::custom("duration out of range"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negative_one_and_a_half_seconds() {
        let delta = TimeDelta::try_milliseconds(-1500).unwrap();
        let (secs, nanos) = split_delta(delta);
        assert_eq!(secs, -2);
        assert_eq!(nanos, 500_000_000);
        assert_eq!(combine_delta(secs, nanos), Some(delta));
    }

    #[test]
    fn test_split_and_combine_roundtrip() {
        for (secs, nanos) in [
            (0, 0),
            (1, 0),
            (-1, 0),
            (90, 0),
            (-90, 0),
            (0, 1),
            (-1, 999_999_999),
            (12345, 678_900_000),
            (-12345, 678_900_000),
        ] {
            let combined = combine_delta(secs, nanos).unwrap();
            assert_eq!(split_delta(combined), (secs, nanos));
        }
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Wrapper(#[serde(with = "super")] TimeDelta);

    #[test]
    fn test_json_backward_compatibility() {
        let json_str = r#"{"secs": 90, "nanos": 0}"#;
        let delta: Wrapper = serde_json::from_str(json_str).unwrap();
        assert_eq!(delta.0, TimeDelta::try_seconds(90).unwrap());

        let reserialized = serde_json::to_string(&delta).unwrap();
        assert_eq!(reserialized, r#"{"secs":90,"nanos":0}"#);
    }

    #[test]
    fn test_json_negative_serialization() {
        let delta = Wrapper(TimeDelta::try_milliseconds(-1500).unwrap());
        let json_str = serde_json::to_string(&delta).unwrap();
        assert_eq!(json_str, r#"{"secs":-2,"nanos":500000000}"#);

        let deserialized: Wrapper = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized, delta);
    }
}
