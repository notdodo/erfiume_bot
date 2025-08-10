use super::Region;
use crate::{BoxError, dynamodb, station::Station};
use dynamodb::DynamoDbClient;
use reqwest::Client as HTTPClient;

#[allow(dead_code)]
pub struct EmiliaRomagna;

#[allow(unused_variables)]
impl Region for EmiliaRomagna {
    async fn name(&self) -> &'static str {
        "Emilia-Romagna"
    }

    async fn dynamodb_table(&self) -> &'static str {
        "EmiliaRomagna-Stations"
    }

    async fn fetch_stations_data(
        &self,
        http_client: &HTTPClient,
        dynamodb_client: &DynamoDbClient,
    ) -> Result<Station, BoxError> {
        todo!()
    }
}
