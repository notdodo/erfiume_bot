use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client as AWSClient;
use dynamodb::DynamoDbClient;
use lambda_runtime::{Error as LambdaError, LambdaEvent, service_fn};
use reqwest::Client as HTTPClient;
use serde_json::Value;
use std::error::Error as StdError;
use std::time::Duration;
use tracing::instrument;
use tracing_subscriber::EnvFilter;

use crate::region::Region;
mod dynamodb;
mod region;
mod station;

type BoxError = Box<dyn StdError + Send + Sync>;

#[instrument(skip(dynamodb_client))]
async fn lambda_handler(
    http_client: &HTTPClient,
    dynamodb_client: &DynamoDbClient,
    _: LambdaEvent<Value>,
) -> Result<Value, LambdaError> {
    let results = region::emilia_romagna::EmiliaRomagna
        .fetch_stations_data(http_client, dynamodb_client)
        .await?;
    Ok(serde_json::to_value(results)?)
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
