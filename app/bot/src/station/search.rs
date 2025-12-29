use super::Station;
use anyhow::{Result, anyhow};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use erfiume_dynamodb::UNKNOWN_THRESHOLD;
use erfiume_dynamodb::stations::{StationRecord, get_station_record, list_stations};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use strsim::jaro_winkler;

static STATION_CACHE: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationMatch {
    Exact,
    Fuzzy,
}

fn fuzzy_search(search: &str, stations: &[String]) -> Option<String> {
    const MIN_SCORE: f64 = 0.8;
    let search_lower = search.to_lowercase();
    stations
        .iter()
        .map(|s| {
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
    page_size: i32,
) -> Result<Option<Station>> {
    get_station_with_match(client, station_name, table_name, page_size)
        .await
        .map(|result| result.map(|(station, _)| station))
}

pub async fn get_station_with_match(
    client: &DynamoDbClient,
    station_name: String,
    table_name: &str,
    page_size: i32,
) -> Result<Option<(Station, StationMatch)>> {
    if let Some(record) = get_station_record(client, table_name, &station_name).await? {
        return Ok(Some((record_to_station(record), StationMatch::Exact)));
    }

    let stations = list_stations_cached(client, table_name, page_size).await?;
    if let Some(closest_match) = fuzzy_search(&station_name, &stations) {
        let record = get_station_record(client, table_name, &closest_match).await?;
        match record {
            Some(record) => Ok(Some((record_to_station(record), StationMatch::Fuzzy))),
            None => Err(anyhow!("Station '{}' not found", closest_match)),
        }
    } else {
        Err(anyhow!("'{}' did not match any know station", station_name))
    }
}

pub async fn list_stations_cached(
    client: &DynamoDbClient,
    table_name: &str,
    page_size: i32,
) -> Result<Vec<String>> {
    if let Some(cached) = get_cached_station_names(table_name) {
        return Ok(cached);
    }

    let names = list_stations(client, table_name, page_size).await?;
    set_cached_station_names(table_name, names.clone());
    Ok(names)
}

fn station_cache() -> &'static Mutex<HashMap<String, Vec<String>>> {
    STATION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_cached_station_names(table_name: &str) -> Option<Vec<String>> {
    let cache = station_cache().lock().ok()?;
    cache.get(table_name).cloned()
}

fn set_cached_station_names(table_name: &str, names: Vec<String>) {
    if let Ok(mut cache) = station_cache().lock() {
        cache.insert(table_name.to_string(), names);
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
        value: record.value.unwrap_or(UNKNOWN_THRESHOLD),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_search_cesena_yields_cesena_station() {
        let message = "cesena".to_string();
        let expected = Some("Cesena".to_string());
        let stations = vec!["Cesena".to_string(), "S. Carlo".to_string()];

        assert_eq!(fuzzy_search(&message, &stations), expected);
    }

    #[test]
    fn fuzzy_search_scarlo_yields_scarlo_station() {
        let message = "scarlo".to_string();
        let expected = Some("S. Carlo".to_string());
        let stations = vec!["Cesena".to_string(), "S. Carlo".to_string()];

        assert_eq!(fuzzy_search(&message, &stations), expected);
    }

    #[test]
    fn fuzzy_search_nonexisting_yields_nonexisting_station() {
        let message = "thisdoesnotexists".to_string();
        let expected = None;
        let stations = vec!["Cesena".to_string(), "S. Carlo".to_string()];

        assert_eq!(fuzzy_search(&message, &stations), expected);
    }

    #[test]
    fn fuzzy_search_ecsena_yields_cesena_station() {
        let message = "ecsena".to_string();
        let expected = Some("Cesena".to_string());
        let stations = vec!["Cesena".to_string(), "S. Carlo".to_string()];

        assert_eq!(fuzzy_search(&message, &stations), expected);
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
        assert_eq!(station.value, UNKNOWN_THRESHOLD);
    }
}
