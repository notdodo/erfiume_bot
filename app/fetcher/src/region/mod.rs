use crate::{dynamodb, region::emilia_romagna::EmiliaRomagna};
use dynamodb::DynamoDbClient;
use reqwest::Client as HTTPClient;
use serde::Serialize;
pub mod emilia_romagna;

pub type RegionError = Box<dyn std::error::Error + Send + Sync>;

pub trait Region {
    fn name(&self) -> &'static str;
    fn dynamodb_table(&self) -> &'static str;
    async fn fetch_stations_data(
        &self,
        http_client: &HTTPClient,
        dynamodb_client: &DynamoDbClient,
    ) -> Result<RegionResult, RegionError>;
}

#[derive(Serialize)]
pub struct RegionResult {
    message: String,
    stations_found: usize,
    stations_updated: usize,
    errors: usize,
    status_code: i64,
}

pub enum Regions {
    EmiliaRomagna(EmiliaRomagna),
}
impl Region for Regions {
    fn name(&self) -> &'static str {
        match self {
            Regions::EmiliaRomagna(r) => r.name(),
        }
    }

    fn dynamodb_table(&self) -> &'static str {
        match self {
            Regions::EmiliaRomagna(r) => r.dynamodb_table(),
        }
    }

    async fn fetch_stations_data(
        &self,
        http_client: &HTTPClient,
        dynamodb_client: &DynamoDbClient,
    ) -> Result<RegionResult, RegionError> {
        match self {
            Regions::EmiliaRomagna(r) => r.fetch_stations_data(http_client, dynamodb_client).await,
        }
    }
}
