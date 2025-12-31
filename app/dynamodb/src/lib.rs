pub mod alerts;
pub mod chats;
mod parse;
pub mod stations;
pub mod utils;
pub use parse::{
    parse_number_field, parse_optional_number_field, parse_optional_string_field,
    parse_string_field,
};

pub const ALERT_ACTIVE: &str = "1";
pub const ALERT_TRIGGERED: &str = "0";
/// Sentinel value for missing or unavailable threshold levels.
pub const UNKNOWN_THRESHOLD: f64 = -9999.0;
