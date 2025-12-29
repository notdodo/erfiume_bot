use super::Region;
use crate::{
    alerts::{self, AlertsConfig},
    logging,
    region::{RegionError, RegionResult},
    station::{Entry, Station, StationData},
};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use erfiume_dynamodb::stations::{StationRecord, put_station_record};
use futures::StreamExt;
use reqwest::Client as HTTPClient;
use std::sync::OnceLock;

pub struct EmiliaRomagna;

const API_BASE_URL: &str = "https://allertameteo.regione.emilia-romagna.it";
const LATEST_TIME_SEED: i64 = 1_726_667_100_000;
const SENSOR_VALUES_PATH: &str = "/o/api/allerta/get-sensor-values-no-time";
const TIME_SERIES_PATH: &str = "/o/api/allerta/get-time-series/";
const VARIABILE_PARAM: &str = "variabile=254,0,0/1,-,-,-/B13215";

fn round_two_decimals(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}

#[allow(unused_variables)]
impl Region for EmiliaRomagna {
    fn name(&self) -> &'static str {
        "Emilia-Romagna"
    }

    fn dynamodb_table(&self) -> &'static str {
        emilia_romagna_table_name()
    }

    async fn fetch_stations_data(
        &self,
        http_client: &HTTPClient,
        dynamodb_client: &DynamoDbClient,
    ) -> Result<RegionResult, RegionError> {
        let api_base = API_BASE_URL;
        let latest_timestamp = fetch_latest_time(http_client, api_base).await?;
        let stations = fetch_stations(http_client, api_base, latest_timestamp).await?;
        let stations_count = stations.len();
        let concurrency_limit = 40;
        let alerts_config = AlertsConfig::from_env();

        let process_futures = stations.into_iter().map(|station| {
            process_station(
                http_client,
                dynamodb_client,
                api_base,
                station,
                self.dynamodb_table(),
                alerts_config.as_ref(),
            )
        });

        let process_results: Vec<_> = futures::stream::iter(process_futures)
            .buffer_unordered(concurrency_limit)
            .collect()
            .await;

        let successful_updates = process_results.iter().filter(|res| res.is_ok()).count();
        let error_count = process_results.iter().filter(|res| res.is_err()).count();
        for result in process_results {
            if let Err(e) = result {
                let logger = logging::Logger::new().error_text(e.to_string());
                logger.error("stations.process_failed", &e, "Error processing station");
            }
        }

        Ok(RegionResult {
            message: format!(
                "Processed {} of {} stations",
                successful_updates, stations_count
            ),
            stations_found: stations_count,
            stations_updated: successful_updates,
            errors: error_count,
            status_code: if error_count > 0 { 206 } else { 200 },
        })
    }
}

fn emilia_romagna_table_name() -> &'static str {
    static TABLE_NAME: OnceLock<String> = OnceLock::new();
    TABLE_NAME
        .get_or_init(|| {
            std::env::var("EMILIA_ROMAGNA_STATIONS_TABLE_NAME")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| panic!("Missing env var: EMILIA_ROMAGNA_STATIONS_TABLE_NAME"))
        })
        .as_str()
}

async fn fetch_latest_time(client: &HTTPClient, api_base: &str) -> Result<i64, RegionError> {
    let url = format!("{api_base}{SENSOR_VALUES_PATH}?{VARIABILE_PARAM}&time={LATEST_TIME_SEED}");
    let response = client.get(url).send().await?;

    response.error_for_status_ref()?;

    let entries: Vec<Entry> = response.json().await?;
    for entry in entries {
        if let Entry::TimeEntry { time } = entry {
            let timestamp = time
                .parse::<i64>()
                .map_err(|e| format!("Failed to parse 'time': {e}"))?;
            return Ok(timestamp);
        }
    }

    Err("No 'TimeEntry' found in response".into())
}

async fn fetch_stations(
    client: &HTTPClient,
    api_base: &str,
    timestamp: i64,
) -> Result<Vec<Station>, RegionError> {
    let url = format!("{api_base}{SENSOR_VALUES_PATH}?{VARIABILE_PARAM}&time={timestamp}");
    let response = client.get(&url).send().await?;
    response.error_for_status_ref()?;

    let entries: Vec<Entry> = response.json().await?;
    let stations = entries
        .into_iter()
        .filter_map(|e| match e {
            Entry::DataEntry {
                idstazione,
                ordinamento,
                nomestaz,
                lon,
                soglia1,
                value: _,
                soglia2,
                lat,
                soglia3,
                timestamp: _,
            } => Some(Station {
                idstazione,
                ordinamento,
                nomestaz,
                lon,
                soglia1: round_two_decimals(soglia1),
                soglia2: round_two_decimals(soglia2),
                soglia3: round_two_decimals(soglia3),
                lat,
                timestamp: None,
                value: None,
            }),
            Entry::TimeEntry { .. } => None,
        })
        .collect();
    Ok(stations)
}

async fn fetch_station_data(
    client: &HTTPClient,
    api_base: &str,
    mut station: Station,
) -> Result<Station, RegionError> {
    let url = format!(
        "{api_base}{TIME_SERIES_PATH}?stazione={}&{VARIABILE_PARAM}",
        station.idstazione,
    );
    let response = client.get(&url).send().await?;
    response.error_for_status_ref()?;
    let entries: Vec<StationData> = response.json().await?;
    if let Some(latest_value) = entries.iter().max_by_key(|e| e.t) {
        station.timestamp = Some(latest_value.t);
        station.value = latest_value.v.map(round_two_decimals);
    }

    Ok(station)
}

async fn process_station(
    client: &HTTPClient,
    dynamodb_client: &DynamoDbClient,
    api_base: &str,
    station: Station,
    table_name: &str,
    alerts_config: Option<&AlertsConfig>,
) -> Result<(), RegionError> {
    let station = fetch_station_data(client, api_base, station.clone())
        .await
        .inspect_err(|e| {
            let logger = logging::Logger::new().station(&station.nomestaz);
            logger.error(
                "stations.fetch_failed",
                &e,
                "Error fetching data for station",
            );
        })?;

    if let Some(config) = alerts_config
        && let Err(err) =
            alerts::process_alerts_for_station(client, dynamodb_client, &station, config).await
    {
        let logger = logging::Logger::new().station(&station.nomestaz);
        logger.error("alerts.process_failed", &err, "Failed to process alerts");
        return Err(err.into());
    }

    let record = StationRecord {
        timestamp: station.timestamp.unwrap_or_default() as i64,
        idstazione: station.idstazione.clone(),
        ordinamento: station.ordinamento,
        nomestaz: station.nomestaz.clone(),
        lon: station.lon.clone(),
        lat: station.lat.clone(),
        soglia1: station.soglia1 as f64,
        soglia2: station.soglia2 as f64,
        soglia3: station.soglia3 as f64,
        value: station.value.map(|value| value as f64),
    };
    put_station_record(dynamodb_client, table_name, &record).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_two_decimals_rounds_as_expected() {
        let value = round_two_decimals(1.235);
        assert!((value - 1.24).abs() < 1e-6);
    }
}
