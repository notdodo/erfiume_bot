use super::Region;
use crate::{
    alerts::{self, AlertsConfig},
    logging,
    region::{RegionError, RegionResult},
    station::{Entry, Station, StationData},
};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use chrono::{Duration, Utc};
use erfiume_dynamodb::stations::{StationRecord, put_station_record};
use futures::StreamExt;
use reqwest::Client as HTTPClient;
use serde_json::Value;
use std::sync::OnceLock;

pub struct EmiliaRomagna;

const API_BASE_URL: &str = "https://allertameteo.regione.emilia-romagna.it";
const LATEST_TIME_SEED: i64 = 1_726_667_100_000;
const SENSOR_VALUES_PATH: &str = "/o/api/allerta/get-sensor-values-no-time";
const TIME_SERIES_PATH: &str = "/o/api/allerta/get-time-series/";
const VARIABILE_PARAM: &str = "variabile=254,0,0/1,-,-,-/B13215";
const GRAFICO_PATH: &str = "/web/guest/grafico-sensori";
const GRAFICO_VARIABILE: &str = "254,0,0/1,-,-,-/B13215";

#[derive(Clone)]
struct EmiliaRomagnaMeta {
    bacino: Option<String>,
}

fn round_two_decimals(value: f64) -> f64 {
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
                bacino: None,
                provincia: None,
                comune: None,
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
    let mut station = fetch_station_data(client, api_base, station.clone())
        .await
        .inspect_err(|e| {
            let logger = logging::Logger::new().station(&station.nomestaz);
            logger.error(
                "stations.fetch_failed",
                &e,
                "Error fetching data for station",
            );
        })?;

    let meta = match fetch_station_metadata(client, api_base, &station.idstazione).await {
        Ok(meta) => meta,
        Err(err) => {
            let logger = logging::Logger::new().station(&station.nomestaz);
            logger.error(
                "stations.metadata_failed",
                &err,
                "Failed to load station metadata",
            );
            None
        }
    };

    if let Some(meta) = meta.as_ref() {
        station.bacino = meta.bacino.clone();
        station.provincia = None;
        station.comune = None;
    }

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
        soglia1: station.soglia1,
        soglia2: station.soglia2,
        soglia3: station.soglia3,
        bacino: station.bacino.clone(),
        provincia: station.provincia.clone(),
        comune: station.comune.clone(),
        value: station.value,
    };
    put_station_record(dynamodb_client, table_name, &record).await?;

    Ok(())
}

async fn fetch_station_metadata(
    client: &HTTPClient,
    api_base: &str,
    station_id: &str,
) -> Result<Option<EmiliaRomagnaMeta>, RegionError> {
    let end = Utc::now().date_naive();
    let start = end - Duration::days(2);
    let fmt = "%Y-%m-%d";
    let start = start.format(fmt).to_string();
    let end = end.format(fmt).to_string();
    let r_param = format!("{station_id}/{GRAFICO_VARIABILE}/{start}/{end}");
    let url = format!(
        "{api_base}{GRAFICO_PATH}?p_p_id=AllertaGraficoPortlet&p_p_lifecycle=0&\
_AllertaGraficoPortlet_mvcRenderCommandName=%2Fallerta%2Fanimazione%2Fgrafico&\
r={r_param}&stazione={station_id}&variabile={GRAFICO_VARIABILE}"
    );

    let response = client.get(&url).send().await?;
    response.error_for_status_ref()?;
    let payload = response.text().await?;
    Ok(parse_grafico_metadata(&payload))
}

fn parse_grafico_metadata(payload: &str) -> Option<EmiliaRomagnaMeta> {
    let marker = payload
        .find("var  data")
        .or_else(|| payload.find("var data"))?;
    let json = extract_json_object(&payload[marker..])?;
    let value: Value = serde_json::from_str(&json).ok()?;

    let bacino = value
        .get("namebasin")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| {
            value
                .get("namesubbasin")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });

    let bacino = bacino?;
    Some(EmiliaRomagnaMeta {
        bacino: Some(bacino),
    })
}

fn extract_json_object(payload: &str) -> Option<String> {
    let start = payload.find('{')?;
    let mut depth = 0i32;
    for (offset, ch) in payload[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = start + offset + 1;
                    return Some(payload[start..end].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_two_decimals_rounds_as_expected() {
        let value = round_two_decimals(1.235);
        assert!((value - 1.24).abs() < 1e-6);
    }

    #[test]
    fn parse_grafico_metadata_extracts_bacino() {
        let html = r#"
        <script type="text/javascript">
        var  data = {"unit":"M","namebasin":"SAVIO","name":"Cesena","description":"Livello idrometrico","namesubbasin":"SAVIO","soglia1":4.0,"soglia2":5.5,"soglia3":7.8,"height":31.0};
        </script>
        "#;
        let meta = parse_grafico_metadata(html).expect("expected metadata");
        assert_eq!(meta.bacino, Some("SAVIO".to_string()));
    }
}
