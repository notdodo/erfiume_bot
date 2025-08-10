use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client as AWSClient;
use dynamodb::DynamoDbClient;
use futures::StreamExt;
use lambda_runtime::{Error as LambdaError, LambdaEvent, service_fn};
use reqwest::Client as HTTPClient;
use serde_json::{Value, json};
use station::{Entry, Station, StationData};
use std::error::Error as StdError;
use std::time::Duration;
use tracing::{error, instrument};
use tracing_subscriber::EnvFilter;
mod dynamodb;
mod region;
mod station;

type BoxError = Box<dyn StdError + Send + Sync>;

async fn fetch_latest_time(client: &HTTPClient) -> Result<i64, BoxError> {
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
                .map_err(|e| format!("Failed to parse 'time': {e}"))?;
            return Ok(timestamp);
        }
    }

    Err("No 'TimeEntry' found in response".into())
}

async fn fetch_stations(client: &HTTPClient, timestamp: i64) -> Result<Vec<Station>, BoxError> {
    let url = format!(
        "https://allertameteo.regione.emilia-romagna.it/o/api/allerta/get-sensor-values-no-time?variabile=254,0,0/1,-,-,-/B13215&time={timestamp}"
    );
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
    client: &HTTPClient,
    mut station: Station,
) -> Result<Station, BoxError> {
    let url = format!(
        "https://allertameteo.regione.emilia-romagna.it/o/api/allerta/get-time-series/?stazione={}&variabile=254,0,0/1,-,-,-/B13215",
        station.idstazione
    );
    let response = client.get(&url).send().await?;
    response.error_for_status_ref()?;
    let entries: Vec<StationData> = response.json().await?;
    if let Some(latest_value) = entries.iter().max_by_key(|e| e.t) {
        station.timestamp = Some(latest_value.t);
        station.value = latest_value.v;
    }

    Ok(station)
}

async fn process_station(
    client: &HTTPClient,
    dynamodb_client: &DynamoDbClient,
    station: Station,
) -> Result<(), BoxError> {
    let station = fetch_station_data(client, station.clone())
        .await
        .map_err(|e| {
            error!(
                "Error fetching data for station {}: {:?}",
                station.nomestaz, e
            );
            e
        });
    dynamodb_client
        .put_station_into_dynamodb(&station?, "EmiliaRomagna-Stations")
        .await?;

    Ok(())
}

#[instrument(skip(dynamodb_client))]
async fn lambda_handler(
    http_client: &HTTPClient,
    dynamodb_client: &DynamoDbClient,
    _: LambdaEvent<Value>,
) -> Result<Value, LambdaError> {
    let latest_timestamp = fetch_latest_time(http_client).await?;
    let stations = fetch_stations(http_client, latest_timestamp).await?;
    let concurrency_limit = 40;

    let process_futures = stations
        .clone()
        .into_iter()
        .map(|station| process_station(http_client, dynamodb_client, station));

    let process_results: Vec<_> = futures::stream::iter(process_futures)
        .buffer_unordered(concurrency_limit)
        .collect()
        .await;

    let successful_updates = process_results.iter().filter(|res| res.is_ok()).count();
    let mut error_count = 0;
    for result in process_results {
        if let Err(e) = result {
            error_count += 1;
            error!(error = %e, "Error processing station: {:?}", e);
        }
    }
    if error_count > 0 {
        error!(%error_count, "Aborting Lambda execution due to errors");
        return Err(anyhow::anyhow!("Processing failed for {} stations", error_count).into());
    }

    Ok(json!({
        "message": "Lambda executed successfully",
        "stations_found": stations.len(),
        "stations_updated": successful_updates,
        "errors": error_count,
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
        .without_time()
        .init();

    let http_client = HTTPClient::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let dynamodb_client = DynamoDbClient::new(AWSClient::new(
        &aws_config::defaults(BehaviorVersion::latest()).load().await,
    ));

    lambda_runtime::run(service_fn(|event: LambdaEvent<Value>| async {
        lambda_handler(&http_client, &dynamodb_client, event).await
    }))
    .await?;
    Ok(())
}
