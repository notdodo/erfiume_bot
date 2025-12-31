use super::Region;
use crate::alerts::{self, AlertsConfig};
use crate::logging;
use crate::region::{RegionError, RegionResult};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use chrono::{Duration, Utc};
use chrono_tz::Europe::Rome;
use erfiume_dynamodb::UNKNOWN_THRESHOLD;
use erfiume_dynamodb::stations::{StationRecord, put_station_record};
use reqwest::Client as HTTPClient;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration as StdDuration;

pub struct Marche;

const SESSION_ID: &str = "erfiume";
const MAX_SENSORS: usize = 5;
const LATEST_LOOKBACK_HOURS: i64 = 24;
const THRESHOLD_LOOKBACK_DAYS: i64 = 365 * 3;
const MARCHE_MENU_URL: &str =
    "http://app.protezionecivile.marche.it/sol/annaliidro2/menu.sol?lang=it";
const MARCHE_INDEX_URL: &str =
    "http://app.protezionecivile.marche.it/sol/annaliidro2/index.sol?lang=it";
const MARCHE_QUERY_URL: &str =
    "http://app.protezionecivile.marche.it/sol/annaliidro2/queryResultsFile.sol?lang=it";
const MARCHE_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
const MARCHE_REQUEST_TIMEOUT_SECS: u64 = 90;
const MARCHE_TIMESTEP_TYPE: &str = "y";
const MARCHE_TIMESTEP_VALUE: &str = "999";
const MARCHE_COOKIE_HEADER: &str = "displayCookieConsent=y; PHPSESSID=erfiume";
const MARCHE_ORIGIN: &str = "http://app.protezionecivile.marche.it";

struct MarcheSensor {
    id_raw: String,
    id_rt: String,
    name: String,
}

#[derive(Default, Clone)]
struct MarcheStationMeta {
    bacino: Option<String>,
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
        let alerts_config = AlertsConfig::from_env();
        let html = fetch_menu_html(http_client).await?;
        let sensors = parse_station_options(&html);
        let max_per_request = MAX_SENSORS;
        let end = Utc::now().with_timezone(&Rome);
        let start = end - Duration::hours(LATEST_LOOKBACK_HOURS);
        let fmt = "%Y-%m-%d %H:%M";
        let begin = start.format(fmt).to_string();
        let end = end.format(fmt).to_string();

        let mut series_values = HashMap::new();
        for (index, chunk) in sensors.chunks(max_per_request).enumerate() {
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
            let chunk_values = extract_latest_values(series);
            for (id, value) in chunk_values {
                series_values.insert(id, value);
            }
        }

        let station_meta = match fetch_station_metadata(http_client).await {
            Ok(meta) => meta,
            Err(err) => {
                logging::Logger::new().error(
                    "marche.metadata.failed",
                    &err,
                    "Failed to collect station metadata",
                );
                HashMap::new()
            }
        };

        let threshold_end = Utc::now().with_timezone(&Rome);
        let threshold_start = threshold_end - Duration::days(THRESHOLD_LOOKBACK_DAYS);
        let fmt = "%Y-%m-%d %H:%M";
        let threshold_begin = threshold_start.format(fmt).to_string();
        let threshold_end = threshold_end.format(fmt).to_string();
        let mut max_thresholds = HashMap::new();
        for (index, chunk) in sensors.chunks(max_per_request).enumerate() {
            let chunk_thresholds =
                match fetch_thresholds_chunk(http_client, chunk, &threshold_begin, &threshold_end)
                    .await
                {
                    Ok(thresholds) => thresholds,
                    Err(err) => {
                        logging::Logger::new().error(
                            "marche.thresholds.failed",
                            &err,
                            &format!("Failed to fetch threshold chunk {}", index + 1),
                        );
                        continue;
                    }
                };
            for (id, value) in chunk_thresholds {
                max_thresholds.insert(id, value);
            }
        }

        let mut updated = 0usize;
        for (index, sensor) in sensors.iter().enumerate() {
            let Some((timestamp, value)) = series_values.get(&sensor.id_raw) else {
                continue;
            };
            let max_threshold = max_thresholds.get(&sensor.id_raw).copied();
            let meta = station_meta.get(&sensor.id_raw);
            let station = crate::station::Station {
                timestamp: Some((*timestamp).max(0) as u64),
                idstazione: sensor.id_rt.clone(),
                ordinamento: (index + 1) as i32,
                nomestaz: sensor.name.clone(),
                lon: "0".to_string(),
                lat: "0".to_string(),
                soglia1: UNKNOWN_THRESHOLD,
                soglia2: UNKNOWN_THRESHOLD,
                soglia3: max_threshold.unwrap_or(UNKNOWN_THRESHOLD),
                bacino: meta.and_then(|value| value.bacino.clone()),
                value: Some(*value),
            };

            if let Some(config) = alerts_config.as_ref()
                && let Err(err) = alerts::process_alerts_for_station(
                    http_client,
                    dynamodb_client,
                    &station,
                    config,
                )
                .await
            {
                let logger = logging::Logger::new().station(&station.nomestaz);
                logger.error("alerts.process_failed", &err, "Failed to process alerts");
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
                value: station.value,
            };

            match put_station_record(dynamodb_client, self.dynamodb_table(), &record).await {
                Ok(()) => {
                    updated += 1;
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
    let response = marche_request(http_client, MARCHE_MENU_URL)
        .header(reqwest::header::REFERER, MARCHE_INDEX_URL)
        .form(&{
            let params = vec![
                ("sessid", SESSION_ID.to_string()),
                ("TipoDato", "idrodata".to_string()),
                ("TimeSeriesType", "0".to_string()),
                ("Idrometri_query", "0".to_string()),
                ("SelezionaBacino", "All".to_string()),
                ("SelezionaProvincia", "All".to_string()),
                ("SelezionaComune", "All".to_string()),
                ("submit_basin", "Seleziona".to_string()),
            ];
            params
        })
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
    let mut params = Vec::with_capacity(10 + sensors.len());
    params.push(("sessid", SESSION_ID.to_string()));
    params.push(("outputType", "plot".to_string()));
    params.push(("TipoDato", "original".to_string()));
    params.push(("TipoTabella", "Livelli_".to_string()));
    params.push(("BeginDate", begin.to_string()));
    params.push(("EndDate", end.to_string()));
    params.push(("TimeStepType", MARCHE_TIMESTEP_TYPE.to_string()));
    params.push(("TimeStep", MARCHE_TIMESTEP_VALUE.to_string()));
    params.push(("LineNumberPdf", "0".to_string()));
    for sensor in sensors {
        params.push(("SelezionaStazione[]", sensor.id_raw.clone()));
    }

    let response = marche_request(http_client, MARCHE_QUERY_URL)
        .header(reqwest::header::REFERER, MARCHE_MENU_URL)
        .form(&params)
        .send()
        .await?;
    response.error_for_status_ref()?;
    let payload = response.text().await?;
    serde_json::from_str(&payload).map_err(|err| err.into())
}

async fn fetch_thresholds_chunk(
    http_client: &HTTPClient,
    sensors: &[MarcheSensor],
    begin: &str,
    end: &str,
) -> Result<HashMap<String, f64>, RegionError> {
    let mut params = Vec::with_capacity(8 + sensors.len());
    params.push(("sessid", SESSION_ID.to_string()));
    params.push(("outputType", "file".to_string()));
    params.push(("TipoDato", "original".to_string()));
    params.push(("TipoTabella", "minMaxLV".to_string()));
    params.push(("BeginDate", begin.to_string()));
    params.push(("EndDate", end.to_string()));
    params.push(("LineNumberPdf", "0".to_string()));
    for sensor in sensors {
        params.push(("SelezionaStazione[]", sensor.id_raw.clone()));
    }

    let response = marche_request(http_client, MARCHE_QUERY_URL)
        .header(reqwest::header::REFERER, MARCHE_MENU_URL)
        .form(&params)
        .send()
        .await?;
    response.error_for_status_ref()?;
    let payload = response.text().await?;
    Ok(parse_minmax_response(&payload))
}

fn extract_latest_values(series: Vec<MarcheSeries>) -> HashMap<String, (i64, f64)> {
    let now_ms = Utc::now().timestamp_millis();
    let mut values = HashMap::new();
    for entry in series {
        let Some(sensor_id) = extract_sensor_id_from_series_name(&entry.name) else {
            continue;
        };
        if let Some((timestamp, value)) = latest_valid_point(&entry.data, now_ms) {
            if value < 0.0 && value.abs() <= 0.1 {
                let tail = format_series_tail(&entry.data, 3);
                logging::Logger::new().station(&entry.name).info(
                    "marche.series.latest_suspicious",
                    &format!("sensor_id={sensor_id} latest=({timestamp},{value}) tail={tail}"),
                );
            }
            values.insert(sensor_id, (timestamp, value));
        }
    }
    values
}

fn latest_valid_point(data: &[(i64, Option<f64>)], now_ms: i64) -> Option<(i64, f64)> {
    data.iter()
        .filter_map(|(timestamp, value)| value.map(|value| (*timestamp, value)))
        .filter(|(timestamp, _)| *timestamp <= now_ms)
        .max_by_key(|(timestamp, _)| *timestamp)
}

fn format_series_tail(data: &[(i64, Option<f64>)], count: usize) -> String {
    let mut points: Vec<(i64, f64)> = data
        .iter()
        .filter_map(|(timestamp, value)| value.map(|value| (*timestamp, value)))
        .collect();
    points.sort_by_key(|(timestamp, _)| *timestamp);
    let tail: Vec<String> = points
        .into_iter()
        .rev()
        .take(count)
        .collect::<Vec<(i64, f64)>>()
        .into_iter()
        .rev()
        .map(|(timestamp, value)| format!("({timestamp},{value})"))
        .collect();
    format!("[{}]", tail.join(", "))
}

async fn fetch_station_metadata(
    http_client: &HTTPClient,
) -> Result<HashMap<String, MarcheStationMeta>, RegionError> {
    let index_html = fetch_index_html(http_client).await?;
    let bacini = parse_select_options(&index_html, "SelezionaBacino");

    let mut metadata: HashMap<String, MarcheStationMeta> = HashMap::new();
    for bacino in bacini {
        let stations_html = fetch_menu_html_filtered(http_client, &bacino).await?;
        for sensor in parse_station_options(&stations_html) {
            set_station_meta(&mut metadata, &sensor.id_raw, |meta| {
                if meta.bacino.is_none() {
                    meta.bacino = Some(bacino.clone());
                }
            });
        }
    }

    Ok(metadata)
}

async fn fetch_index_html(http_client: &HTTPClient) -> Result<String, RegionError> {
    let response = marche_request(http_client, MARCHE_INDEX_URL)
        .header(reqwest::header::REFERER, MARCHE_INDEX_URL)
        .form(&{
            let params = vec![("sessid", SESSION_ID.to_string())];
            params
        })
        .send()
        .await?;
    response.error_for_status_ref()?;
    Ok(response.text().await?)
}

async fn fetch_menu_html_filtered(
    http_client: &HTTPClient,
    bacino: &str,
) -> Result<String, RegionError> {
    let response = marche_request(http_client, MARCHE_MENU_URL)
        .header(reqwest::header::REFERER, MARCHE_INDEX_URL)
        .form(&{
            let params = vec![
                ("sessid", SESSION_ID.to_string()),
                ("TipoDato", "idrodata".to_string()),
                ("TimeSeriesType", "0".to_string()),
                ("Idrometri_query", "0".to_string()),
                ("SelezionaBacino", bacino.to_string()),
                ("SelezionaProvincia", "All".to_string()),
                ("SelezionaComune", "All".to_string()),
                ("submit_basin", "Seleziona".to_string()),
            ];
            params
        })
        .send()
        .await?;
    response.error_for_status_ref()?;
    Ok(response.text().await?)
}

fn marche_request(http_client: &HTTPClient, url: &str) -> reqwest::RequestBuilder {
    http_client
        .post(url)
        .header(reqwest::header::USER_AGENT, MARCHE_USER_AGENT)
        .header(reqwest::header::ORIGIN, MARCHE_ORIGIN)
        .header(reqwest::header::COOKIE, MARCHE_COOKIE_HEADER)
        .timeout(StdDuration::from_secs(MARCHE_REQUEST_TIMEOUT_SECS))
}

fn set_station_meta(
    metadata: &mut HashMap<String, MarcheStationMeta>,
    sensor_id: &str,
    update: impl FnOnce(&mut MarcheStationMeta),
) {
    let entry = metadata.entry(sensor_id.to_string()).or_default();
    update(entry);
}

fn parse_minmax_response(payload: &str) -> HashMap<String, f64> {
    let mut thresholds = HashMap::new();
    for line in payload.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Codice sensore") {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(|field| field.trim()).collect();
        if fields.len() < 15 {
            continue;
        }
        let sensor_id = fields[0];
        if sensor_id.is_empty() {
            continue;
        }
        let Some(max_value) = parse_numeric_field(fields[14]) else {
            continue;
        };
        if max_value <= UNKNOWN_THRESHOLD {
            continue;
        }
        thresholds.insert(sensor_id.to_string(), max_value);
    }
    thresholds
}

fn parse_numeric_field(value: &str) -> Option<f64> {
    let normalized = value.trim().replace(',', ".");
    normalized.parse::<f64>().ok()
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

fn parse_select_options(html: &str, select_name: &str) -> Vec<String> {
    let marker_double = format!("name=\"{select_name}\"");
    let marker_single = format!("name='{select_name}'");
    let Some(start) = html
        .find(&marker_double)
        .or_else(|| html.find(&marker_single))
    else {
        return Vec::new();
    };
    let select_block = &html[start..];
    let end = select_block.find("</select>").unwrap_or(select_block.len());
    let content = &select_block[..end];
    let mut values = Vec::new();
    for chunk in content.split("<option").skip(1) {
        let value = if let Some((_, rest)) = chunk.split_once("value=\"") {
            rest.split_once('"').map(|(value, _)| value.trim())
        } else if let Some((_, rest)) = chunk.split_once("value='") {
            rest.split_once('\'').map(|(value, _)| value.trim())
        } else {
            None
        };
        let label = chunk
            .split_once('>')
            .and_then(|(_, rest)| rest.split_once("</option>"));
        let label = label.map(|(value, _)| value.trim()).unwrap_or("");
        let candidate = value.filter(|val| !val.is_empty()).unwrap_or(label);
        if candidate.is_empty() || candidate.eq_ignore_ascii_case("all") {
            continue;
        }
        if !values.iter().any(|existing| existing == candidate) {
            values.push(candidate.to_string());
        }
    }
    values
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
        let data = vec![(3, Some(0.2)), (1, Some(0.1)), (2, None)];
        assert_eq!(latest_valid_point(&data, 3), Some((3, 0.2)));
    }

    #[test]
    fn parse_series_response_reads_values() {
        let payload = r#"[{"name":"Foo (sensore 123)","data":[[1,0.1],[2,null],[3,0.2]]}]"#;
        let parsed: Vec<MarcheSeries> = serde_json::from_str(payload).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "Foo (sensore 123)");
        assert_eq!(parsed[0].data.len(), 3);
    }

    #[test]
    fn parse_minmax_response_extracts_max_values() {
        let payload = "\
Codice sensore, Data min: Anno, Mese, Giorno, Ora, Minuto, Livello idrometrico min [m], Livello idrometrico interpolato [0/1], Portata minima [m3 s-1], Data max: Anno, Mese, Giorno, Ora, Minuto, Livello idrometrico max [m], Livello idrometrico interpolato [0/1], Portata massima [m3 s-1], Num. valori, Quality level [%], Codice stazione, Scala deflusso ufficiale [0/1]\n\
1040, 2025, 12, 16, 9, 0, 0.12, 0, -9999.00, 2025, 12, 12, 8, 0, 0.16, 0, -9999.00, 736, 50.3, 11, -9999\n\
1185, 2025, 12, 11, 7, 30, -0.25, 0, -9999.00, 2025, 12, 1, 3, 0, -9999.00, 0, -9999.00, 861, 58.9, 106, -9999\n";
        let parsed = parse_minmax_response(payload);
        assert_eq!(parsed.get("1040"), Some(&0.16));
        assert!(!parsed.contains_key("1185"));
    }

    #[test]
    fn parse_select_options_reads_values() {
        let html = r#"
        <select name="SelezionaBacino" id="SelezionaBacino_id">
          <option value="All">Tutti</option>
          <option value="Misa">Misa</option>
          <option value="Esino">Esino</option>
        </select>
        "#;
        let options = parse_select_options(html, "SelezionaBacino");
        assert_eq!(options, vec!["Misa".to_string(), "Esino".to_string()]);
    }
}
