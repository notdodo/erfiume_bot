pub(crate) mod search;
use erfiume_dynamodb::utils::format_station_message;
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
    pub fn create_station_message(&self) -> String {
        format_station_message(
            &self.nomestaz,
            Some(self.value),
            self.soglia1,
            self.soglia2,
            self.soglia3,
            Some(self.timestamp),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use erfiume_dynamodb::UNKNOWN_THRESHOLD;

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
