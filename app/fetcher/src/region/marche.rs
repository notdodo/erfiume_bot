use super::Region;
use crate::logging;
use crate::region::{RegionError, RegionResult};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use chrono::{Duration, TimeZone, Utc};
use chrono_tz::Europe::Rome;
use erfiume_dynamodb::UNKNOWN_THRESHOLD;
use erfiume_dynamodb::stations::{StationRecord, put_station_record};
use reqwest::Client as HTTPClient;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

pub struct Marche;

const SESSION_ID: &str = "erfiume";
const MAX_SENSORS: usize = 5;
const LATEST_LOOKBACK_HOURS: i64 = 24;

struct MarcheSensor {
    id_raw: String,
    id_rt: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct MarcheSeries {
    name: String,
    data: Vec<(i64, Option<f64>)>,
}

impl Region for Marche {
    fn name(&self) -> &'static str {
        "Marche"
    }

    fn dynamodb_table(&self) -> &'static str {
        marche_table_name()
    }

    async fn fetch_stations_data(
        &self,
        http_client: &HTTPClient,
        dynamodb_client: &DynamoDbClient,
    ) -> Result<RegionResult, RegionError> {
        logging::Logger::new().info("marche.fetch.start", "Starting Marche fetch");
        let html = fetch_menu_html(http_client).await?;
        logging::Logger::new().info(
            "marche.menu.fetched",
            &format!("Fetched menu HTML ({} bytes)", html.len()),
        );
        let sensors = parse_station_options(&html);
        logging::Logger::new().info(
            "marche.menu.parsed",
            &format!("Parsed {} sensors", sensors.len()),
        );
        let max_per_request = MAX_SENSORS;
        let (begin, end) = build_date_range();
        logging::Logger::new().info(
            "marche.range.built",
            &format!("Date range: {begin} -> {end}"),
        );

        let mut series_values = HashMap::new();
        for (index, chunk) in sensors.chunks(max_per_request).enumerate() {
            logging::Logger::new().info(
                "marche.series.request",
                &format!("Fetching chunk {} ({} sensors)", index + 1, chunk.len()),
            );
            let series = match fetch_series_chunk(http_client, chunk, &begin, &end).await {
                Ok(series) => series,
                Err(err) => {
                    logging::Logger::new().error(
                        "marche.series.failed",
                        &err,
                        &format!("Failed to fetch chunk {}", index + 1),
                    );
                    return Err(err);
                }
            };
            logging::Logger::new().info(
                "marche.series.response",
                &format!("Chunk {} returned {} series", index + 1, series.len()),
            );
            let chunk_values = extract_latest_values(series);
            logging::Logger::new().info(
                "marche.series.extracted",
                &format!(
                    "Chunk {} yielded {} latest values",
                    index + 1,
                    chunk_values.len()
                ),
            );
            for (id, value) in chunk_values {
                series_values.insert(id, value);
            }
        }
        logging::Logger::new().info(
            "marche.series.collected",
            &format!("Collected {} values", series_values.len()),
        );

        let mut updated = 0usize;
        for (index, sensor) in sensors.iter().enumerate() {
            let Some((timestamp, value)) = series_values.get(&sensor.id_raw) else {
                logging::Logger::new()
                    .station(&sensor.name)
                    .info("marche.series.missing", "Missing series data for sensor");
                continue;
            };
            let record = StationRecord {
                timestamp: *timestamp,
                idstazione: sensor.id_rt.clone(),
                ordinamento: (index + 1) as i32,
                nomestaz: sensor.name.clone(),
                lon: "0".to_string(),
                lat: "0".to_string(),
                soglia1: UNKNOWN_THRESHOLD,
                soglia2: UNKNOWN_THRESHOLD,
                soglia3: UNKNOWN_THRESHOLD,
                value: Some(*value),
            };

            match put_station_record(dynamodb_client, self.dynamodb_table(), &record).await {
                Ok(()) => {
                    updated += 1;
                    logging::Logger::new().station(&sensor.name).info(
                        "marche.station.saved",
                        &format!("Stored station {}", sensor.id_rt),
                    );
                }
                Err(err) => {
                    logging::Logger::new().station(&sensor.name).error(
                        "marche.station.save_failed",
                        &err,
                        &format!("Failed to store station {}", sensor.id_rt),
                    );
                }
            }
        }
        logging::Logger::new().info(
            "marche.fetch.complete",
            &format!("Updated {} of {} stations", updated, sensors.len()),
        );

        Ok(RegionResult {
            message: format!("Processed {} of {} stations", updated, sensors.len()),
            stations_found: sensors.len(),
            stations_updated: updated,
            errors: sensors.len().saturating_sub(updated),
            status_code: if updated < sensors.len() { 206 } else { 200 },
        })
    }
}

fn marche_table_name() -> &'static str {
    static TABLE_NAME: OnceLock<String> = OnceLock::new();
    TABLE_NAME
        .get_or_init(|| {
            std::env::var("MARCHE_STATIONS_TABLE_NAME")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| panic!("Missing env var: MARCHE_STATIONS_TABLE_NAME"))
        })
        .as_str()
}

async fn fetch_menu_html(http_client: &HTTPClient) -> Result<String, RegionError> {
    let response = http_client
        .post("http://app.protezionecivile.marche.it/sol/annaliidro2/menu.sol?lang=it")
        .form(&menu_form_params())
        .send()
        .await?;
    response.error_for_status_ref()?;
    Ok(response.text().await?)
}

async fn fetch_series_chunk(
    http_client: &HTTPClient,
    sensors: &[MarcheSensor],
    begin: &str,
    end: &str,
) -> Result<Vec<MarcheSeries>, RegionError> {
    let mut params = Vec::with_capacity(8 + sensors.len());
    params.push(("sessid", SESSION_ID.to_string()));
    params.push(("outputType", "plot".to_string()));
    params.push(("TipoDato", "original".to_string()));
    params.push(("TipoTabella", "Livelli_".to_string()));
    params.push(("BeginDate", begin.to_string()));
    params.push(("EndDate", end.to_string()));
    params.push(("LineNumberPdf", "0".to_string()));
    for sensor in sensors {
        params.push(("SelezionaStazione[]", sensor.id_raw.clone()));
    }

    let response = http_client
        .post("http://app.protezionecivile.marche.it/sol/annaliidro2/queryResultsFile.sol?lang=it")
        .form(&params)
        .send()
        .await?;
    response.error_for_status_ref()?;
    let payload = response.text().await?;
    parse_series_response(&payload).map_err(|err| err.into())
}

fn extract_latest_values(series: Vec<MarcheSeries>) -> HashMap<String, (i64, f64)> {
    let mut values = HashMap::new();
    for entry in series {
        let Some(sensor_id) = extract_sensor_id_from_series_name(&entry.name) else {
            continue;
        };
        if let Some((timestamp, value)) = latest_valid_point(&entry.data) {
            values.insert(sensor_id, (timestamp, value));
        }
    }
    values
}

fn latest_valid_point(data: &[(i64, Option<f64>)]) -> Option<(i64, f64)> {
    data.iter()
        .rev()
        .find_map(|(timestamp, value)| value.map(|value| (*timestamp, value)))
}

fn parse_series_response(payload: &str) -> Result<Vec<MarcheSeries>, serde_json::Error> {
    serde_json::from_str(payload)
}

fn extract_sensor_id_from_series_name(name: &str) -> Option<String> {
    let marker = "(sensore ";
    let start = name.find(marker)? + marker.len();
    let end = name[start..].find(')')? + start;
    let id = name[start..end].trim();
    (!id.is_empty()).then(|| id.to_string())
}

fn parse_station_options(html: &str) -> Vec<MarcheSensor> {
    let mut sensors = Vec::new();
    for chunk in html.split("<option value=\"").skip(1) {
        let Some((id, rest)) = chunk.split_once("\">") else {
            continue;
        };
        let Some((label, _)) = rest.split_once("</option>") else {
            continue;
        };
        let name = extract_station_name(label);
        if !id.trim().is_empty() && !name.is_empty() {
            sensors.push(MarcheSensor {
                id_raw: id.trim().to_string(),
                id_rt: format!("RT-{}", id.trim()),
                name,
            });
        }
    }
    sensors
}

fn extract_station_name(label: &str) -> String {
    let trimmed = label.trim();
    if let Some((name, _)) = trimmed.split_once(" (RT-") {
        return name.trim().to_string();
    }
    if let Some((name, _)) = trimmed.split_once(" Dati da") {
        return name.trim().to_string();
    }
    trimmed.to_string()
}

fn menu_form_params() -> Vec<(&'static str, String)> {
    vec![
        ("sessid", SESSION_ID.to_string()),
        ("TipoDato", "idrodata".to_string()),
        ("TimeSeriesType", "0".to_string()),
        ("Idrometri_query", "0".to_string()),
        ("SelezionaBacino", "All".to_string()),
        ("SelezionaProvincia", "All".to_string()),
        ("SelezionaComune", "All".to_string()),
        ("submit_basin", "Seleziona".to_string()),
    ]
}

fn build_date_range() -> (String, String) {
    let end = Rome.from_utc_datetime(&Utc::now().naive_utc());
    let start = end - Duration::hours(LATEST_LOOKBACK_HOURS);
    let fmt = "%Y-%m-%d %H:%M";
    (start.format(fmt).to_string(), end.format(fmt).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_station_options_extracts_ids_and_names() {
        let html = r#"
        <select id="SelezionaStazione_id" name="SelezionaStazione[]" multiple="multiple">
        <option value="1040">Abbadia di Fiastra (RT-1040) Dati da 2000-07-07 a 2025-12-29</option>
        <option value="1185">Acqualagna (RT-1185) Dati da 2003-06-06 a 2025-12-29</option>
        </select>
        "#;
        let parsed = parse_station_options(html);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id_raw, "1040");
        assert_eq!(parsed[0].id_rt, "RT-1040");
        assert_eq!(parsed[0].name, "Abbadia di Fiastra");
    }

    #[test]
    fn extract_station_name_fallbacks() {
        assert_eq!(
            extract_station_name("Foo (RT-123) Dati da 2000-01-01"),
            "Foo"
        );
        assert_eq!(extract_station_name("Bar Dati da 2000-01-01"), "Bar");
        assert_eq!(extract_station_name("Baz"), "Baz");
    }

    #[test]
    fn extract_sensor_id_from_series_name_parses_id() {
        assert_eq!(
            extract_sensor_id_from_series_name("Abbadia di Fiastra (sensore 1040)"),
            Some("1040".to_string())
        );
    }

    #[test]
    fn latest_valid_point_skips_nulls() {
        let data = vec![(1, Some(0.1)), (2, None), (3, Some(0.2))];
        assert_eq!(latest_valid_point(&data), Some((3, 0.2)));
    }

    #[test]
    fn parse_series_response_reads_values() {
        let payload = r#"[{"name":"Foo (sensore 123)","data":[[1,0.1],[2,null],[3,0.2]]}]"#;
        let parsed = parse_series_response(payload).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "Foo (sensore 123)");
        assert_eq!(parsed[0].data.len(), 3);
    }
}
