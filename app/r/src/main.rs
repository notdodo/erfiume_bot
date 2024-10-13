use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use futures::StreamExt;
use lambda_runtime::{service_fn, Error as LambdaError, LambdaEvent};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::error::Error as StdError;
use std::fmt;
use std::time::Duration;
use tracing::{error, info, instrument};
use tracing_subscriber::EnvFilter;

type BoxError = Box<dyn StdError + Send + Sync>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum Entry {
    TimeEntry {
        time: String,
    },
    DataEntry {
        idstazione: String,
        ordinamento: i32,
        nomestaz: String,
        lon: String,
        soglia1: f32,
        value: Option<String>,
        soglia2: f32,
        lat: String,
        soglia3: f32,
        timestap: Option<u64>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Station {
    timestamp: Option<u64>,
    idstazione: String,
    ordinamento: i32,
    nomestaz: String,
    lon: String,
    lat: String,
    soglia1: f32,
    soglia2: f32,
    soglia3: f32,
    value: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct StationData {
    #[serde(deserialize_with = "deserialize_timestamp")]
    t: u64,
    v: Option<f32>,
}

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct TimestampVisitor;

    impl<'de> Visitor<'de> for TimestampVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a u64 or a string representing a u64")
        }

        fn visit_u64<E>(self, value: u64) -> Result<u64, E> {
            Ok(value)
        }

        fn visit_str<E>(self, value: &str) -> Result<u64, E>
        where
            E: de::Error,
        {
            value.parse::<u64>().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(TimestampVisitor)
}

async fn fetch_latest_time(client: &reqwest::Client) -> Result<i64, BoxError> {
    let response = client
        .get("https://allertameteo.regione.emilia-romagna.it/o/api/allerta/get-sensor-values-no-time?variabile=254,0,0/1,-,-,-/B13215&time=1726667100000")
        .send()
        .await?;

    response.error_for_status_ref()?;

    let entries: Vec<Entry> = response.json().await?;
    for entry in entries {
        if let Entry::TimeEntry { time } = entry {
            let timestamp = time
                .parse::<i64>()
                .map_err(|e| format!("Failed to parse 'time': {}", e))?;
            return Ok(timestamp);
        }
    }

    Err("No 'TimeEntry' found in response".into())
}

async fn fetch_stations(
    client: &reqwest::Client,
    timestamp: i64,
) -> Result<Vec<Station>, BoxError> {
    let url = format!("https://allertameteo.regione.emilia-romagna.it/o/api/allerta/get-sensor-values-no-time?variabile=254,0,0/1,-,-,-/B13215&time={}", timestamp);
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
                timestap: _,
            } => Some(Station {
                idstazione,
                ordinamento,
                nomestaz,
                lon,
                soglia1,
                soglia2,
                soglia3,
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
    client: &reqwest::Client,
    mut station: Station,
) -> Result<Station, BoxError> {
    let url = format!("https://allertameteo.regione.emilia-romagna.it/o/api/allerta/get-time-series/?stazione={}&variabile=254,0,0/1,-,-,-/B13215", station.idstazione);
    let response = client.get(&url).send().await?;
    response.error_for_status_ref()?;
    let entries: Vec<StationData> = response.json().await?;
    if let Some(latest_value) = entries.iter().max_by_key(|e| e.t) {
        station.timestamp = Some(latest_value.t);
        station.value = latest_value.v;
    }

    Ok(station)
}

async fn put_station_into_dynamodb(
    client: &DynamoDbClient,
    station: &Station,
    table_name: &str,
) -> Result<(), BoxError> {
    let mut item = std::collections::HashMap::new();

    item.insert(
        "idstazione".to_string(),
        AttributeValue::S(station.idstazione.clone()),
    );
    item.insert(
        "ordinamento".to_string(),
        AttributeValue::N(station.ordinamento.to_string()),
    );
    item.insert(
        "nomestaz".to_string(),
        AttributeValue::S(station.nomestaz.clone()),
    );
    item.insert("lon".to_string(), AttributeValue::S(station.lon.clone()));
    item.insert("lat".to_string(), AttributeValue::S(station.lat.clone()));
    item.insert(
        "soglia1".to_string(),
        AttributeValue::N(station.soglia1.to_string()),
    );
    item.insert(
        "soglia2".to_string(),
        AttributeValue::N(station.soglia2.to_string()),
    );
    item.insert(
        "soglia3".to_string(),
        AttributeValue::N(station.soglia3.to_string()),
    );
    if let Some(value) = station.value {
        item.insert("value".to_string(), AttributeValue::N(value.to_string()));
    }
    if let Some(timestamp) = station.timestamp {
        item.insert(
            "timestamp".to_string(),
            AttributeValue::N(timestamp.to_string()),
        );
    }

    client
        .put_item()
        .table_name(table_name)
        .set_item(Some(item))
        .send()
        .await?;

    Ok(())
}

async fn process_station(
    client: &reqwest::Client,
    dynamodb_client: &DynamoDbClient,
    station: Station,
    table_name: &str,
) -> Result<(), BoxError> {
    let station = fetch_station_data(client, station).await?;
    put_station_into_dynamodb(dynamodb_client, &station, table_name).await?;

    Ok(())
}

#[instrument]
async fn lambda_handler(_: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let shared_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
    let dynamodb_client = DynamoDbClient::new(&shared_config);

    let latest_timestamp = fetch_latest_time(&http_client).await?;
    let stations = fetch_stations(&http_client, latest_timestamp).await?;

    let concurrency_limit = 100;

    let process_futures = stations
        .clone()
        .into_iter()
        .map(|station| process_station(&http_client, &dynamodb_client, station, "Stazioni"));

    let process_results: Vec<_> = futures::stream::iter(process_futures)
        .buffer_unordered(concurrency_limit)
        .collect()
        .await;

    let mut successful_updates = 0;
    for result in process_results {
        match result {
            Ok(_) => successful_updates += 1,
            Err(e) => error!(error = %e, "Error processing station: {:?}", e),
        }
    }

    info!(
        successful_updates = successful_updates,
        total_stations = stations.len(),
        "Finished processing stations"
    );
    Ok(json!({
        "message": "Lambda executed successfully",
        "stations_processed": stations.len(),
        "stations_updated": successful_updates,
        "statusCode": 200,
    }))
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()) // Enable log level filtering via `RUST_LOG` env var
        .json()
        .with_current_span(false) // Optional: Exclude span information
        .with_span_list(false) // Optional: Exclude span list
        .with_target(false) // Optional: Exclude target (module path)
        .without_time() // AWS Lambda adds timestamps, so you can exclude them
        .init();

    let func = service_fn(lambda_handler);
    lambda_runtime::run(func).await?;
    Ok(())
}
