use anyhow::Result;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Entry {
    TimeEntry {
        time: String,
    },
    DataEntry {
        idstazione: String,
        ordinamento: i32,
        nomestaz: String,
        lon: String,
        soglia1: f32,
        value: Option<String>,
        soglia2: f32,
        lat: String,
        soglia3: f32,
        timestamp: Option<u64>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Station {
    pub timestamp: Option<u64>,
    pub idstazione: String,
    pub ordinamento: i32,
    pub nomestaz: String,
    pub lon: String,
    pub lat: String,
    pub soglia1: f32,
    pub soglia2: f32,
    pub soglia3: f32,
    pub value: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct StationData {
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub t: u64,
    pub v: Option<f32>,
}
fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct TimestampVisitor;

    impl Visitor<'_> for TimestampVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a u64 or a string representing a u64")
        }

        fn visit_u64<E>(self, value: u64) -> Result<u64, E> {
            Ok(value)
        }

        fn visit_str<E>(self, value: &str) -> Result<u64, E>
        where
            E: de::Error,
        {
            value.parse::<u64>().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(TimestampVisitor)
}
