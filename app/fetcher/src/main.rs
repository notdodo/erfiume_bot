use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client as AWSClient;
use dynamodb::DynamoDbClient;
use futures::future::join_all;
use lambda_runtime::{Error as LambdaError, LambdaEvent, service_fn};
use region::{Region, Regions, emilia_romagna::EmiliaRomagna};
use reqwest::Client as HTTPClient;
use serde_json::{Value, json};
use std::time::Duration;
use tracing::instrument;
use tracing_subscriber::EnvFilter;
mod dynamodb;
mod region;
mod station;

#[instrument(skip(dynamodb_client, regions))]
async fn lambda_handler(
    http_client: &HTTPClient,
    dynamodb_client: &DynamoDbClient,
    regions: &[Regions],
    _: LambdaEvent<Value>,
) -> Result<Value, LambdaError> {
    let futures = regions.iter().map(|region| {
        let region_name = region.name();
        async move {
            match region
                .fetch_stations_data(http_client, dynamodb_client)
                .await
            {
                Ok(result) => json!({
                    "region": region_name,
                    "status": "ok",
                    "result": result
                }),
                Err(e) => json!({
                    "region": region_name,
                    "status": "error",
                    "error": e.to_string()
                }),
            }
        }
    });
    let aggregated = join_all(futures).await;

    Ok(json!({"regions": aggregated}))
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
    let regions: Vec<Regions> = vec![Regions::EmiliaRomagna(EmiliaRomagna)];

    lambda_runtime::run(service_fn(|event: LambdaEvent<Value>| async {
        lambda_handler(&http_client, &dynamodb_client, &regions, event).await
    }))
    .await?;
    Ok(())
}
