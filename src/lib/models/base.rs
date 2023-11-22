use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct MetaData {
    pub created_by: Uuid,
    #[serde(with = "my_date_format")]
    pub created_on: DateTime<Utc>,
    pub modified_by: Uuid,
    #[serde(with = "my_date_format")]
    pub modified_on: DateTime<Utc>,
    pub deleted_by: Uuid,
    #[serde(with = "my_date_format")]
    pub deleted_on: DateTime<Utc>,
    // data: T,
}

mod my_date_format {
    use chrono::{DateTime, FixedOffset, ParseError, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    const FORMAT: &'static str = "%Y-%m-%d %H:%M:%S";

    // The signature of a serialize_with function must follow the pattern:
    //
    //    fn serialize<S>(&T, S) -> Result<S::Ok, S::Error>
    //    where
    //        S: Serializer
    //
    // although it may also be generic over the input types T.
    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", date.format(FORMAT));
        serializer.serialize_str(&s)
    }

    // The signature of a deserialize_with function must follow the pattern:
    //
    //    fn deserialize<'de, D>(D) -> Result<T, D::Error>
    //    where
    //        D: Deserializer<'de>
    //
    // although it may also be generic over the output types T.
    pub fn _deserialize<'de, D>(deserializer: D) -> Result<DateTime<FixedOffset>, ParseError>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer).unwrap();
        DateTime::parse_from_str(&s, FORMAT)
    }
}
