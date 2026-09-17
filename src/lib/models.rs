pub mod custom_claims;
pub mod finance;
pub mod meta;
pub mod person;
pub mod post;
pub mod tax_info;
pub mod token;

/// Serde helper for bare calendar dates (`YYYY-MM-DD`).
///
/// The rest of the project uses `time::OffsetDateTime` with
/// `#[serde(with = "time::serde::iso8601")]`, but that helper is
/// `OffsetDateTime`-only. Ledger fields — when income was received, when an
/// expense was incurred — are genuinely calendar days rather than instants,
/// because tax periods break on calendar-day boundaries.
pub mod date_iso {
    use serde::{Deserialize, Deserializer, Serializer};
    use time::format_description::FormatItem;
    use time::macros::format_description;
    use time::Date;

    const FMT: &[FormatItem<'_>] = format_description!("[year]-[month]-[day]");

    pub fn serialize<S: Serializer>(date: &Date, s: S) -> Result<S::Ok, S::Error> {
        let formatted = date.format(&FMT).map_err(serde::ser::Error::custom)?;
        s.serialize_str(&formatted)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Date, D::Error> {
        let raw = String::deserialize(d)?;
        Date::parse(&raw, &FMT).map_err(serde::de::Error::custom)
    }

    // No `Option<Date>` field exists yet, so there's no `option` submodule
    // here the way `time::serde::iso8601` ships one — add one the same
    // shape as `serialize`/`deserialize` above (swap `Date` for
    // `Option<Date>`, match on it) if a nullable date field shows up.
}
