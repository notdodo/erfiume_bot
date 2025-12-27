use super::{Station, UNKNOWN_VALUE, stations};
use anyhow::{Result, anyhow};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use erfiume_dynamodb::stations::{StationRecord, get_station_record};
use strsim::jaro_winkler;

fn fuzzy_search(search: &str) -> Option<String> {
    const MIN_SCORE: f64 = 0.8;
    let stations = stations();
    let search_lower = search.to_lowercase();
    stations
        .iter()
        .map(|s: &String| {
            let s_normalized = s.replace(" ", "").to_lowercase();
            let score = jaro_winkler(&search_lower, &s_normalized);
            (s, score)
        })
        .filter(|(_, score)| *score > MIN_SCORE) // Adjust the threshold as needed
        .max_by(|(_, score_a), (_, score_b)| score_a.partial_cmp(score_b).unwrap())
        .map(|(station, _)| station.clone())
}

pub async fn get_station(
    client: &DynamoDbClient,
    station_name: String,
    table_name: &str,
) -> Result<Option<Station>> {
    if let Some(closest_match) = fuzzy_search(&station_name) {
        let record = get_station_record(client, table_name, &closest_match).await?;
        match record {
            Some(record) => Ok(Some(record_to_station(record))),
            None => Err(anyhow!("Station '{}' not found", closest_match)),
        }
    } else {
        Err(anyhow!("'{}' did not match any know station", station_name))
    }
}

fn record_to_station(record: StationRecord) -> Station {
    Station {
        timestamp: record.timestamp,
        idstazione: record.idstazione,
        ordinamento: record.ordinamento,
        nomestaz: record.nomestaz,
        lon: record.lon,
        lat: record.lat,
        soglia1: record.soglia1,
        soglia2: record.soglia2,
        soglia3: record.soglia3,
        value: record.value.unwrap_or(UNKNOWN_VALUE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_search_cesena_yields_cesena_station() {
        let message = "cesena".to_string();
        let expected = Some("Cesena".to_string());

        assert_eq!(fuzzy_search(&message), expected);
    }

    #[test]
    fn fuzzy_search_scarlo_yields_scarlo_station() {
        let message = "scarlo".to_string();
        let expected = Some("S. Carlo".to_string());

        assert_eq!(fuzzy_search(&message), expected);
    }

    #[test]
    fn fuzzy_search_nonexisting_yields_nonexisting_station() {
        let message = "thisdoesnotexists".to_string();
        let expected = None;

        assert_eq!(fuzzy_search(&message), expected);
    }

    #[test]
    fn fuzzy_search_ecsena_yields_cesena_station() {
        let message = "ecsena".to_string();
        let expected = Some("Cesena".to_string());

        assert_eq!(fuzzy_search(&message), expected);
    }

    #[test]
    fn record_to_station_uses_unknown_value_on_missing() {
        let record = StationRecord {
            timestamp: 1,
            idstazione: "id".to_string(),
            ordinamento: 1,
            nomestaz: "Cesena".to_string(),
            lon: "lon".to_string(),
            lat: "lat".to_string(),
            soglia1: 1.0,
            soglia2: 2.0,
            soglia3: 3.0,
            value: None,
        };
        let station = record_to_station(record);
        assert_eq!(station.value, UNKNOWN_VALUE);
    }
}
