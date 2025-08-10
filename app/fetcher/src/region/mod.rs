use crate::dynamodb;
use dynamodb::DynamoDbClient;
use reqwest::Client as HTTPClient;
pub mod emilia_romagna;
use lambda_runtime::Error as LambdaError;
use serde::{Deserialize, Serialize};

pub trait Region {
    fn dynamodb_table(&self) -> &'static str;
    async fn fetch_stations_data(
        &self,
        http_client: &HTTPClient,
        dynamodb_client: &DynamoDbClient,
    ) -> Result<RegionResult, LambdaError>;
}

#[derive(Serialize, Deserialize)]
pub struct RegionResult {
    message: String,
    stations_found: usize,
    stations_updated: usize,
    errors: i64,
    status_code: i64,
}
