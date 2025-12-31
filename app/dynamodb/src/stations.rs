use crate::{
    UNKNOWN_THRESHOLD, parse_number_field, parse_optional_number_field,
    parse_optional_string_field, parse_string_field,
};
use anyhow::{Result, anyhow};
use aws_sdk_dynamodb::{
    Client, error::SdkError, operation::update_item::UpdateItemError, types::AttributeValue,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct StationRecord {
    pub timestamp: i64,
    pub idstazione: String,
    pub ordinamento: i32,
    pub nomestaz: String,
    pub lon: String,
    pub lat: String,
    pub soglia1: f64,
    pub soglia2: f64,
    pub soglia3: f64,
    pub bacino: Option<String>,
    pub provincia: Option<String>,
    pub comune: Option<String>,
    pub value: Option<f64>,
}

pub async fn get_station_record(
    client: &Client,
    table_name: &str,
    station_name: &str,
) -> Result<Option<StationRecord>> {
    if table_name.is_empty() {
        return Err(anyhow!("stations table name is empty"));
    }

    let result = client
        .get_item()
        .table_name(table_name)
        .key("nomestaz", AttributeValue::S(station_name.to_string()))
        .send()
        .await?;

    let Some(item) = result.item else {
        return Ok(None);
    };

    let idstazione = parse_string_field(&item, "idstazione")?;
    let timestamp = parse_number_field::<i64>(&item, "timestamp")?;
    let lon = parse_string_field(&item, "lon")?;
    let lat = parse_string_field(&item, "lat")?;
    let ordinamento = parse_number_field::<i32>(&item, "ordinamento")?;
    let nomestaz = parse_string_field(&item, "nomestaz")?;
    let soglia1 = parse_number_field::<f64>(&item, "soglia1")?;
    let soglia2 = parse_number_field::<f64>(&item, "soglia2")?;
    let soglia3 = parse_number_field::<f64>(&item, "soglia3")?;
    let bacino = parse_optional_string_field(&item, "bacino")?;
    let provincia = parse_optional_string_field(&item, "provincia")?;
    let comune = parse_optional_string_field(&item, "comune")?;
    let value = parse_optional_number_field::<f64>(&item, "value")?;

    Ok(Some(StationRecord {
        timestamp,
        idstazione,
        ordinamento,
        nomestaz,
        lon,
        lat,
        soglia1,
        soglia2,
        soglia3,
        bacino,
        provincia,
        comune,
        value,
    }))
}

pub async fn put_station_record(
    client: &Client,
    table_name: &str,
    station: &StationRecord,
) -> Result<()> {
    if table_name.is_empty() {
        return Err(anyhow!("stations table name is empty"));
    }

    let new_timestamp = station.timestamp;
    let new_value = station.value.unwrap_or_default();

    let mut expression_attribute_values = HashMap::from([
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

    let update_soglia3 = station.soglia3 != UNKNOWN_THRESHOLD;
    if update_soglia3 {
        expression_attribute_values.insert(
            ":unknown".to_string(),
            AttributeValue::N(UNKNOWN_THRESHOLD.to_string()),
        );
    }

    let expression_attribute_names = HashMap::from([
        ("#tsp".to_string(), "timestamp".to_string()),
        ("#vl".to_string(), "value".to_string()),
    ]);

    let mut update_parts = vec![
        "#tsp = :new_timestamp",
        "#vl = :new_value",
        "idstazione = if_not_exists(idstazione, :idstazione)",
        "ordinamento = if_not_exists(ordinamento, :ordinamento)",
        "lon = if_not_exists(lon, :lon)",
        "lat = if_not_exists(lat, :lat)",
        "soglia1 = if_not_exists(soglia1, :soglia1)",
        "soglia2 = if_not_exists(soglia2, :soglia2)",
    ];
    if update_soglia3 {
        update_parts.push("soglia3 = :soglia3");
    } else {
        update_parts.push("soglia3 = if_not_exists(soglia3, :soglia3)");
    }

    let mut condition_parts = vec![
        "attribute_not_exists(#tsp)",
        ":new_timestamp > #tsp",
        "attribute_not_exists(idstazione)",
        "attribute_not_exists(ordinamento)",
        "attribute_not_exists(lon)",
        "attribute_not_exists(lat)",
        "attribute_not_exists(soglia1)",
        "attribute_not_exists(soglia2)",
        "attribute_not_exists(soglia3)",
    ];
    if update_soglia3 {
        condition_parts.push("soglia3 = :unknown");
    }

    if let Some(bacino) = station.bacino.as_ref().filter(|value| !value.is_empty()) {
        update_parts.push("bacino = if_not_exists(bacino, :bacino)");
        condition_parts.push("attribute_not_exists(bacino)");
        expression_attribute_values
            .insert(":bacino".to_string(), AttributeValue::S(bacino.to_string()));
    }
    if let Some(provincia) = station.provincia.as_ref().filter(|value| !value.is_empty()) {
        update_parts.push("provincia = if_not_exists(provincia, :provincia)");
        condition_parts.push("attribute_not_exists(provincia)");
        expression_attribute_values.insert(
            ":provincia".to_string(),
            AttributeValue::S(provincia.to_string()),
        );
    }
    if let Some(comune) = station.comune.as_ref().filter(|value| !value.is_empty()) {
        update_parts.push("comune = if_not_exists(comune, :comune)");
        condition_parts.push("attribute_not_exists(comune)");
        expression_attribute_values
            .insert(":comune".to_string(), AttributeValue::S(comune.to_string()));
    }

    let update_expression = format!("SET {}", update_parts.join(", "));
    let condition_expression = condition_parts.join(" OR ");

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
                Ok(())
            } else {
                Err(anyhow::Error::new(err.into_err()))
            }
        }
        Err(err) => Err(err.into()),
    }
}

pub async fn list_stations(
    client: &Client,
    table_name: &str,
    page_size: i32,
) -> Result<Vec<String>> {
    if table_name.is_empty() {
        return Err(anyhow!("stations table name is empty"));
    }

    let page_size = page_size.clamp(1, 100);
    let mut names = Vec::new();
    let mut last_evaluated_key: Option<HashMap<String, AttributeValue>> = None;

    loop {
        let mut request = client
            .scan()
            .table_name(table_name)
            .projection_expression("nomestaz")
            .limit(page_size);

        if let Some(key) = last_evaluated_key.take() {
            request = request.set_exclusive_start_key(Some(key));
        }

        let response = request.send().await?;
        if let Some(items) = response.items {
            for item in items {
                names.push(parse_string_field(&item, "nomestaz")?);
            }
        }

        match response.last_evaluated_key {
            Some(key) if !key.is_empty() => {
                last_evaluated_key = Some(key);
            }
            _ => break,
        }
    }

    names.sort();
    names.dedup();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn station_record_roundtrip_fields() {
        let record = StationRecord {
            timestamp: 123,
            idstazione: "id".to_string(),
            ordinamento: 1,
            nomestaz: "Cesena".to_string(),
            lon: "10.0".to_string(),
            lat: "20.0".to_string(),
            soglia1: 1.0,
            soglia2: 2.0,
            soglia3: 3.0,
            bacino: None,
            provincia: None,
            comune: None,
            value: Some(1.2),
        };

        assert_eq!(record.nomestaz, "Cesena");
        assert_eq!(record.value, Some(1.2));
    }
}
