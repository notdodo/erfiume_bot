use chrono::{DateTime, TimeZone};
use chrono_tz::Europe::Rome;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::UNKNOWN_THRESHOLD;

pub fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn format_station_message(
    station_name: &str,
    value: Option<f64>,
    threshold_yellow: f64,
    threshold_orange: f64,
    threshold_red: f64,
    timestamp_millis: Option<i64>,
) -> String {
    let timestamp_formatted = timestamp_millis
        .and_then(|timestamp| {
            let timestamp_secs = timestamp / 1000;
            let naive_datetime = DateTime::from_timestamp(timestamp_secs, 0)?;
            let datetime_in_tz = Rome.from_utc_datetime(&naive_datetime.naive_utc());
            Some(datetime_in_tz.format("%d-%m-%Y %H:%M").to_string())
        })
        .unwrap_or_else(|| "non disponibile".to_string());

    let value = value.unwrap_or(UNKNOWN_THRESHOLD);
    let thresholds_available = threshold_yellow != UNKNOWN_THRESHOLD
        && threshold_orange != UNKNOWN_THRESHOLD
        && threshold_red != UNKNOWN_THRESHOLD;

    let mut alarm = if thresholds_available {
        if value <= threshold_yellow {
            "🟢"
        } else if value > threshold_yellow && value <= threshold_orange {
            "🟡"
        } else if value >= threshold_orange && value <= threshold_red {
            "🟠"
        } else {
            "🔴"
        }
    } else {
        ""
    };

    let mut value_str = format!("{value:.2}");
    if value == UNKNOWN_THRESHOLD {
        value_str = "non disponibile".to_string();
        alarm = "";
    }

    let mut lines = Vec::with_capacity(6);
    lines.push(format!("Stazione: {}", station_name));
    lines.push(format!("Valore: {} {}", value_str, alarm));
    if threshold_yellow != UNKNOWN_THRESHOLD {
        let yellow_str = format_threshold(threshold_yellow);
        lines.push(format!("Soglia Gialla: {}", yellow_str));
    }
    if threshold_orange != UNKNOWN_THRESHOLD {
        let orange_str = format_threshold(threshold_orange);
        lines.push(format!("Soglia Arancione: {}", orange_str));
    }
    if threshold_red != UNKNOWN_THRESHOLD {
        let red_str = format_threshold(threshold_red);
        lines.push(format!("Soglia Rossa: {}", red_str));
    }
    lines.push(format!("Ultimo rilevamento: {}", timestamp_formatted));

    lines.join("\n")
}

fn format_threshold(value: f64) -> String {
    if value == UNKNOWN_THRESHOLD {
        "non disponibile".to_string()
    } else {
        format!("{value:.2}")
    }
}
