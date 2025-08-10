use super::Region;
use crate::{
    dynamodb,
    region::{RegionError, RegionResult},
    station::{Entry, Station, StationData},
};
use dynamodb::DynamoDbClient;
use futures::StreamExt;
use reqwest::Client as HTTPClient;
use tracing::error;

pub struct EmiliaRomagna;

#[allow(unused_variables)]
impl Region for EmiliaRomagna {
    fn dynamodb_table(&self) -> &'static str {
        "EmiliaRomagna-Stations"
    }

    async fn fetch_stations_data(
        &self,
        http_client: &HTTPClient,
        dynamodb_client: &DynamoDbClient,
    ) -> Result<RegionResult, RegionError> {
        let latest_timestamp = fetch_latest_time(http_client).await?;
        let stations = fetch_stations(http_client, latest_timestamp).await?;
        let stations_count = stations.len();
        let concurrency_limit = 40;

        let process_futures = stations.into_iter().map(|station| {
            process_station(http_client, dynamodb_client, station, self.dynamodb_table())
        });

        let process_results: Vec<_> = futures::stream::iter(process_futures)
            .buffer_unordered(concurrency_limit)
            .collect()
            .await;

        let successful_updates = process_results.iter().filter(|res| res.is_ok()).count();
        let error_count = process_results.iter().filter(|res| res.is_err()).count();
        for result in process_results {
            if let Err(e) = result {
                error!(error = %e, "Error processing station: {:?}", e);
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

async fn fetch_latest_time(client: &HTTPClient) -> Result<i64, RegionError> {
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

async fn fetch_stations(client: &HTTPClient, timestamp: i64) -> Result<Vec<Station>, RegionError> {
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
) -> Result<Station, RegionError> {
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
    table_name: &str,
) -> Result<(), RegionError> {
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
        .put_station_into_dynamodb(&station?, table_name)
        .await?;

    Ok(())
}
