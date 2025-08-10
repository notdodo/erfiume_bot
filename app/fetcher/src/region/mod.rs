use crate::dynamodb;
use dynamodb::DynamoDbClient;
use reqwest::Client as HTTPClient;
pub mod emilia_romagna;
use serde::{Deserialize, Serialize};

pub type RegionError = Box<dyn std::error::Error + Send + Sync>;

pub trait Region {
    fn dynamodb_table(&self) -> &'static str;
    async fn fetch_stations_data(
        &self,
        http_client: &HTTPClient,
        dynamodb_client: &DynamoDbClient,
    ) -> Result<RegionResult, RegionError>;
}

#[derive(Serialize, Deserialize)]
pub struct RegionResult {
    message: String,
    stations_found: usize,
    stations_updated: usize,
    errors: usize,
    status_code: i64,
}
