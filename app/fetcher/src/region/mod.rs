use crate::{BoxError, dynamodb, station::Station};
use dynamodb::DynamoDbClient;
use reqwest::Client as HTTPClient;
pub mod emilia_romagna;

#[allow(dead_code)]
pub trait Region {
    async fn name(&self) -> &'static str;
    async fn dynamodb_table(&self) -> &'static str;

    async fn fetch_station_data(
        &self,
        http_client: &HTTPClient,
        dynamodb_client: &DynamoDbClient,
    ) -> Result<Station, BoxError>;
}
