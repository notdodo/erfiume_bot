pub mod alerts;
mod parse;
pub mod stations;
pub use parse::{parse_number_field, parse_optional_number_field, parse_string_field};

pub const ALERT_ACTIVE: &str = "1";
pub const ALERT_TRIGGERED: &str = "0";
