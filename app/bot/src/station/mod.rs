pub(crate) mod search;
use erfiume_dynamodb::stations::StationListEntry;
use erfiume_dynamodb::utils::format_station_message;
use serde::Deserialize;

pub(crate) const MARCHE_SOGLIA3_NOTICE: &str = "Nota (Marche): la soglia rossa è il massimo storico (ultimi 3 anni) e non una soglia ufficiale.";

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
    bacino: Option<String>,
    value: f64,
}

impl Station {
    pub fn create_station_message(&self) -> String {
        let base_message = format_station_message(
            &self.nomestaz,
            Some(self.value),
            self.soglia1,
            self.soglia2,
            self.soglia3,
            Some(self.timestamp),
        );

        let metadata_lines = self.metadata_lines();
        if metadata_lines.is_empty() {
            return base_message;
        }

        let mut lines: Vec<String> = base_message.lines().map(str::to_string).collect();
        let mut insert_index = 1;
        for line in metadata_lines {
            lines.insert(insert_index, line);
            insert_index += 1;
        }

        lines.join("\n")
    }

    fn metadata_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(bacino) = self.bacino.as_ref().filter(|value| !value.is_empty()) {
            lines.push(format!("Bacino: {}", bacino));
        }
        lines
    }
}

pub(crate) fn format_station_list_entry(entry: &StationListEntry) -> String {
    let mut details = Vec::new();
    if let Some(bacino) = entry.bacino.as_ref().filter(|value| !value.is_empty()) {
        details.push(format!("Bacino: {}", bacino));
    }

    if details.is_empty() {
        entry.nomestaz.clone()
    } else {
        format!("{} - {}", entry.nomestaz, details.join(", "))
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
            bacino: None,
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
            bacino: None,
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
            bacino: None,
            value: 1.2,
        };
        let expected =
            "Stazione: Cesena\nValore: 1.20 \nUltimo rilevamento: 20-10-2024 22:02".to_string();

        assert_eq!(station.create_station_message(), expected);
    }
}
