use anyhow::{Result, anyhow};
use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;

pub fn parse_string_field(item: &HashMap<String, AttributeValue>, field: &str) -> Result<String> {
    match item.get(field) {
        Some(AttributeValue::S(s)) => Ok(s.clone()),
        Some(AttributeValue::Ss(ss)) => Ok(ss.join(",")),
        _ => Err(anyhow!("Missing or invalid '{}' field", field)),
    }
}

pub fn parse_number_field<T: std::str::FromStr>(
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

pub fn parse_optional_number_field<T: std::str::FromStr>(
    item: &HashMap<String, AttributeValue>,
    field: &str,
) -> Result<Option<T>>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match item.get(field) {
        Some(AttributeValue::N(n)) => match n.parse::<T>() {
            Ok(value) => Ok(Some(value)),
            _ => Err(anyhow!(
                "Failed to parse '{}' field with value '{}' as number",
                field,
                n
            )),
        },
        Some(AttributeValue::S(s)) => match s.parse::<T>() {
            Ok(value) => Ok(Some(value)),
            _ => Err(anyhow!(
                "Failed to parse '{}' field with value '{}' as number",
                field,
                s
            )),
        },
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_string_field_yields_value() {
        let expected = "hello".to_string();
        let item = HashMap::from([("field".to_string(), AttributeValue::S(expected.clone()))]);
        assert_eq!(parse_string_field(&item, "field").unwrap(), expected);
    }

    #[test]
    fn parse_number_field_parses_number() {
        let item = HashMap::from([("field".to_string(), AttributeValue::N("42".to_string()))]);
        let parsed: i32 = parse_number_field(&item, "field").unwrap();
        assert_eq!(parsed, 42);
    }

    #[test]
    fn parse_optional_number_field_returns_none_on_missing() {
        let item: HashMap<String, AttributeValue> = HashMap::new();
        let parsed: Option<i64> = parse_optional_number_field(&item, "field").unwrap();
        assert!(parsed.is_none());
    }
}
