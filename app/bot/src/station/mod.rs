pub(crate) mod search;
use chrono::{DateTime, TimeZone};
use chrono_tz::Europe::Rome;
use erfiume_dynamodb::UNKNOWN_THRESHOLD;
use serde::Deserialize;

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct Station {
    timestamp: i64,
    idstazione: String,
    ordinamento: i32,
    pub nomestaz: String,
    lon: String,
    lat: String,
    soglia1: f64,
    soglia2: f64,
    soglia3: f64,
    value: f64,
}

impl Station {
    fn format_threshold(value: f64) -> String {
        if value == UNKNOWN_THRESHOLD {
            "non disponibile".to_string()
        } else {
            format!("{value:.2}")
        }
    }

    pub fn create_station_message(&self) -> String {
        let timestamp_secs = self.timestamp / 1000;
        let naive_datetime = DateTime::from_timestamp(timestamp_secs, 0).unwrap();
        let datetime_in_tz: DateTime<chrono_tz::Tz> =
            Rome.from_utc_datetime(&naive_datetime.naive_utc());
        let timestamp_formatted = datetime_in_tz.format("%d-%m-%Y %H:%M").to_string();

        let value = self.value;

        let yellow = self.soglia1;
        let orange = self.soglia2;
        let red = self.soglia3;

        let thresholds_available =
            yellow != UNKNOWN_THRESHOLD && orange != UNKNOWN_THRESHOLD && red != UNKNOWN_THRESHOLD;

        let mut alarm = "🔴";
        if thresholds_available {
            if value <= yellow {
                alarm = "🟢";
            } else if value > yellow && value <= orange {
                alarm = "🟡";
            } else if value >= orange && value <= red {
                alarm = "🟠";
            }
        } else {
            alarm = "";
        }

        let mut value_str = format!("{value:.2}");
        if value == UNKNOWN_THRESHOLD {
            value_str = "non disponibile".to_string();
            alarm = "";
        }

        let mut lines = Vec::with_capacity(6);
        lines.push(format!("Stazione: {}", self.nomestaz));
        lines.push(format!("Valore: {} {}", value_str, alarm));
        if thresholds_available {
            let yellow_str = Self::format_threshold(yellow);
            let orange_str = Self::format_threshold(orange);
            let red_str = Self::format_threshold(red);
            lines.push(format!("Soglia Gialla: {}", yellow_str));
            lines.push(format!("Soglia Arancione: {}", orange_str));
            lines.push(format!("Soglia Rossa: {}", red_str));
        }
        lines.push(format!("Ultimo rilevamento: {}", timestamp_formatted));

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_station_message_with_unknown_value() {
        let station = Station {
            idstazione: "/id/".to_string(),
            timestamp: 1729454542656,
            ordinamento: 1,
            nomestaz: "Cesena".to_string(),
            lon: "lon".to_string(),
            lat: "lat".to_string(),
            soglia1: 1.0,
            soglia2: 2.0,
            soglia3: 3.0,
            value: UNKNOWN_THRESHOLD,
        };
        let expected = "Stazione: Cesena\nValore: non disponibile \nSoglia Gialla: 1.00\nSoglia Arancione: 2.00\nSoglia Rossa: 3.00\nUltimo rilevamento: 20-10-2024 22:02".to_string();

        assert_eq!(station.create_station_message(), expected);
    }

    #[test]
    fn create_station_message() {
        let station = Station {
            idstazione: "/id/".to_string(),
            timestamp: 1729454542656,
            ordinamento: 1,
            nomestaz: "Cesena".to_string(),
            lon: "lon".to_string(),
            lat: "lat".to_string(),
            soglia1: 1.0,
            soglia2: 2.0,
            soglia3: 3.0,
            value: 2.2,
        };
        let expected = "Stazione: Cesena\nValore: 2.20 🟠\nSoglia Gialla: 1.00\nSoglia Arancione: 2.00\nSoglia Rossa: 3.00\nUltimo rilevamento: 20-10-2024 22:02".to_string();

        assert_eq!(station.create_station_message(), expected);
    }

    #[test]
    fn create_station_message_with_unknown_thresholds() {
        let station = Station {
            idstazione: "/id/".to_string(),
            timestamp: 1729454542656,
            ordinamento: 1,
            nomestaz: "Cesena".to_string(),
            lon: "lon".to_string(),
            lat: "lat".to_string(),
            soglia1: UNKNOWN_THRESHOLD,
            soglia2: UNKNOWN_THRESHOLD,
            soglia3: UNKNOWN_THRESHOLD,
            value: 1.2,
        };
        let expected =
            "Stazione: Cesena\nValore: 1.20 \nUltimo rilevamento: 20-10-2024 22:02".to_string();

        assert_eq!(station.create_station_message(), expected);
    }
}
