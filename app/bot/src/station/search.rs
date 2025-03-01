use anyhow::{anyhow, Result};
use aws_sdk_dynamodb::{types::AttributeValue, Client as DynamoDbClient};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator as _};
use std::collections::HashMap;
use strsim::jaro_winkler;

use super::{stations, Stazione, UNKNOWN_VALUE};

fn fuzzy_search(search: &str) -> Option<String> {
    let stations = stations();
    let search_lower = search.to_lowercase();
    stations
        .par_iter()
        .map(|s: &String| {
            let s_normalized = s.replace(" ", "").to_lowercase();
            let score = jaro_winkler(&search_lower, &s_normalized);
            (s, score)
        })
        .filter(|(_, score)| *score > 0.8) // Adjust the threshold as needed
        .max_by(|(_, score_a), (_, score_b)| score_a.partial_cmp(score_b).unwrap())
        .map(|(station, _)| station.clone())
}

pub async fn get_station(
    client: &DynamoDbClient,
    station_name: String,
    table_name: &str,
) -> Result<Option<Stazione>> {
    if let Some(closest_match) = fuzzy_search(&station_name) {
        let result = client
            .get_item()
            .table_name(table_name)
            .key("nomestaz", AttributeValue::S(closest_match.clone()))
            .send()
            .await?;

        match result.item {
            Some(item) => {
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

                Ok(Some(Stazione {
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
                }))
            }
            None => Err(anyhow!("Station '{}' not found", closest_match)),
        }
    } else {
        Err(anyhow!("'{}' did not match any know station", station_name))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_search_cesena_yields_cesena_station() {
        let message = "cesena".to_string();
        let expected = Some("Cesena".to_string());

        assert_eq!(fuzzy_search(&message), expected);
    }

    #[test]
    fn fuzzy_search_scarlo_yields_scarlo_station() {
        let message = "scarlo".to_string();
        let expected = Some("S. Carlo".to_string());

        assert_eq!(fuzzy_search(&message), expected);
    }

    #[test]
    fn fuzzy_search_nonexisting_yields_nonexisting_station() {
        let message = "thisdoesnotexists".to_string();
        let expected = None;

        assert_eq!(fuzzy_search(&message), expected);
    }

    #[test]
    fn fuzzy_search_ecsena_yields_cesena_station() {
        let message = "ecsena".to_string();
        let expected = Some("Cesena".to_string());

        assert_eq!(fuzzy_search(&message), expected);
    }

    #[test]
    fn parse_string_field_yields_correct_value() {
        let expected = "this is a string".to_string();
        let item = HashMap::from([("field".to_string(), AttributeValue::S(expected.clone()))]);
        assert_eq!(parse_string_field(&item, "field").unwrap(), expected);
    }

    #[test]
    fn parse_optional_number_field_yields_correct_value() {
        let expected = 4;
        let item = HashMap::from([("field".to_string(), AttributeValue::N(expected.to_string()))]);
        assert_eq!(
            parse_optional_number_field::<i16>(&item, "field").unwrap(),
            Some(expected)
        );
    }
}
