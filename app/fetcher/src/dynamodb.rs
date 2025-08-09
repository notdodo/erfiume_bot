use crate::station::Station;
use anyhow::Result;
use aws_sdk_dynamodb::{
    Client as DynamoDbClient, error::SdkError, operation::update_item::UpdateItemError,
    types::AttributeValue,
};
use std::collections::HashMap;

pub async fn put_station_into_dynamodb(
    client: &DynamoDbClient,
    station: &Station,
    table_name: &str,
) -> Result<()> {
    let new_timestamp = station.timestamp.unwrap_or_default();
    let new_value = station.value.unwrap_or_default();

    let expression_attribute_values = HashMap::from([
        (
            ":new_timestamp".to_string(),
            AttributeValue::N(new_timestamp.to_string()),
        ),
        (
            ":new_value".to_string(),
            AttributeValue::N(new_value.to_string()),
        ),
        (
            ":idstazione".to_string(),
            AttributeValue::S(station.idstazione.clone()),
        ),
        (
            ":ordinamento".to_string(),
            AttributeValue::N(station.ordinamento.to_string()),
        ),
        (":lon".to_string(), AttributeValue::S(station.lon.clone())),
        (":lat".to_string(), AttributeValue::S(station.lat.clone())),
        (
            ":soglia1".to_string(),
            AttributeValue::N(station.soglia1.to_string()),
        ),
        (
            ":soglia2".to_string(),
            AttributeValue::N(station.soglia2.to_string()),
        ),
        (
            ":soglia3".to_string(),
            AttributeValue::N(station.soglia3.to_string()),
        ),
    ]);

    let expression_attribute_names = HashMap::from([
        ("#tsp".to_string(), "timestamp".to_string()),
        ("#vl".to_string(), "value".to_string()),
    ]);

    let update_expression = "SET #tsp = :new_timestamp, #vl = :new_value, idstazione = :idstazione, ordinamento = :ordinamento, lon = :lon, lat = :lat, soglia1 = :soglia1, soglia2 = :soglia2, soglia3 = :soglia3";

    let condition_expression = "attribute_not_exists(#tsp) OR :new_timestamp > #tsp";

    let result = client
        .update_item()
        .table_name(table_name)
        .key("nomestaz", AttributeValue::S(station.nomestaz.clone()))
        .update_expression(update_expression)
        .set_expression_attribute_values(Some(expression_attribute_values))
        .set_expression_attribute_names(Some(expression_attribute_names))
        .condition_expression(condition_expression)
        .send()
        .await;

    match result {
        Ok(_) => Ok(()),
        Err(SdkError::ServiceError(err)) => {
            if let UpdateItemError::ConditionalCheckFailedException(_) = err.err() {
                Ok(()) // Skip silently: DynamoDB raise this error when the station exists and there's no new timestamp to update
            } else {
                Err(anyhow::Error::new(err.into_err()))
            }
        }
        Err(err) => Err(err.into()),
    }
}
