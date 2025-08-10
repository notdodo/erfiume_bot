use super::Region;

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

    async fn fetch_station_data(
        &self,
        http_client: &reqwest::Client,
        dynamodb_client: &crate::dynamodb::DynamoDbClient,
    ) -> Result<crate::station::Station, crate::BoxError> {
        todo!()
    }
}
