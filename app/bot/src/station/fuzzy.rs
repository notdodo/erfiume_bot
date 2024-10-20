use anyhow::{anyhow, Result};
use aws_sdk_dynamodb::{types::AttributeValue, Client as DynamoDbClient};
use std::collections::HashMap;

use super::{Stazione, UNKNOWN_VALUE};

pub async fn get_station(
    client: &DynamoDbClient,
    station_name: String,
    table_name: &str,
) -> Result<Stazione> {
    let result = client
        .get_item()
        .table_name(table_name)
        .key("nomestaz", AttributeValue::S(station_name.clone()))
        .send()
        .await?;

    match result.item {
        Some(item) => {
            // Parse each field with proper error handling
            let idstazione = parse_string_field(&item, "idstazione")?;
            let timestamp = parse_number_field::<i64>(&item, "timestamp")?;
            let lon = parse_string_field(&item, "lon")?;
            let lat = parse_string_field(&item, "lat")?;
            let ordinamento = parse_number_field::<i32>(&item, "ordinamento")?;
            let nomestaz = parse_string_field(&item, "nomestaz")?;
            let soglia1 = parse_number_field::<f64>(&item, "soglia1")?;
            let soglia2 = parse_number_field::<f64>(&item, "soglia2")?;
            let soglia3 = parse_number_field::<f64>(&item, "soglia3")?;
            let value = parse_optional_number_field(&item, "value")?.unwrap_or(UNKNOWN_VALUE);

            Ok(Stazione {
                timestamp,
                idstazione,
                ordinamento,
                nomestaz,
                lon,
                lat,
                soglia1,
                soglia2,
                soglia3,
                value,
            })
        }
        None => Err(anyhow!("Station '{}' not found", station_name)),
    }
}

fn parse_string_field(item: &HashMap<String, AttributeValue>, field: &str) -> Result<String> {
    match item.get(field) {
        Some(AttributeValue::S(s)) => Ok(s.clone()),
        Some(AttributeValue::Ss(ss)) => Ok(ss.join(",")), // If the field is a string set
        _ => Err(anyhow!("Missing or invalid '{}' field", field)),
    }
}

fn parse_number_field<T: std::str::FromStr>(
    item: &HashMap<String, AttributeValue>,
    field: &str,
) -> Result<T>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match item.get(field) {
        Some(AttributeValue::N(n)) => n.parse::<T>().map_err(|e| {
            anyhow!(
                "Failed to parse '{}' field with value '{}' as number: {}",
                field,
                n,
                e
            )
        }),
        Some(AttributeValue::S(s)) => s.parse::<T>().map_err(|e| {
            anyhow!(
                "Failed to parse '{}' field with value '{}' as number: {}",
                field,
                s,
                e
            )
        }),
        _ => Err(anyhow!("Missing or invalid '{}' field", field)),
    }
}

fn parse_optional_number_field<T: std::str::FromStr>(
    item: &HashMap<String, AttributeValue>,
    field: &str,
) -> Result<Option<T>>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match item.get(field) {
        Some(AttributeValue::N(n)) => {
            if let Ok(value) = n.parse::<T>() {
                Ok(Some(value))
            } else {
                Err(anyhow!(
                    "Failed to parse '{}' field with value '{}' as number",
                    field,
                    n
                ))
            }
        }
        Some(AttributeValue::S(s)) => {
            if let Ok(value) = s.parse::<T>() {
                Ok(Some(value))
            } else {
                Err(anyhow!(
                    "Failed to parse '{}' field with value '{}' as number",
                    field,
                    s
                ))
            }
        }
        _ => Err(anyhow!("Invalid type for '{}' field", field)),
    }
}
