pub(crate) mod search;
use chrono::{DateTime, TimeZone};
use chrono_tz::Europe::Rome;
use serde::Deserialize;

const UNKNOWN_VALUE: f64 = -9999.0;

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

        let mut alarm = "🔴";
        if value <= yellow {
            alarm = "🟢";
        } else if value > yellow && value <= orange {
            alarm = "🟡";
        } else if value >= orange && value <= red {
            alarm = "🟠";
        }

        let mut value_str = format!("{value:.2}");
        if value == UNKNOWN_VALUE {
            value_str = "non disponibile".to_string();
            alarm = "";
        }

        let yellow_str = format!("{yellow:.2}");
        let orange_str = format!("{orange:.2}");
        let red_str = format!("{red:.2}");

        format!(
            "Stazione: {}\nValore: {} {}\nSoglia Gialla: {}\nSoglia Arancione: {}\nSoglia Rossa: {}\nUltimo rilevamento: {}",
            self.nomestaz, value_str, alarm, yellow_str, orange_str, red_str, timestamp_formatted
        )
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
            value: UNKNOWN_VALUE,
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
}
