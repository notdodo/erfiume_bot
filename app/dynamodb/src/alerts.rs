use crate::{
    ALERT_ACTIVE, ALERT_TRIGGERED, parse_number_field, parse_optional_number_field,
    parse_string_field,
};
use anyhow::{Result, anyhow};
use aws_sdk_dynamodb::{Client, types::AttributeValue};
use std::collections::HashMap;

pub struct AlertEntry {
    pub station_name: String,
    pub threshold: f64,
    pub active: i64,
    pub triggered_at: Option<u64>,
    pub triggered_value: Option<f64>,
}

pub struct AlertSubscription {
    pub chat_id: i64,
    pub threshold: f64,
    pub thread_id: Option<i64>,
}

pub async fn upsert_alert(
    client: &Client,
    table_name: &str,
    station_name: &str,
    chat_id: i64,
    threshold: f64,
    created_at: i64,
    thread_id: Option<i64>,
) -> Result<()> {
    if table_name.is_empty() {
        return Err(anyhow!("alerts table name is empty"));
    }

    let mut expression_attribute_values = HashMap::from([
        (
            ":threshold".to_string(),
            AttributeValue::N(threshold.to_string()),
        ),
        (
            ":created_at".to_string(),
            AttributeValue::N(created_at.to_string()),
        ),
        (
            ":active".to_string(),
            AttributeValue::N(ALERT_ACTIVE.to_string()),
        ),
    ]);

    let mut update_expression =
        "SET threshold = :threshold, created_at = :created_at, active = :active".to_string();
    let mut remove_fields = vec!["triggered_at", "triggered_value"];

    if let Some(thread_id) = thread_id {
        update_expression.push_str(", thread_id = :thread_id");
        expression_attribute_values.insert(
            ":thread_id".to_string(),
            AttributeValue::N(thread_id.to_string()),
        );
    } else {
        remove_fields.push("thread_id");
    }

    if !remove_fields.is_empty() {
        update_expression.push_str(" REMOVE ");
        update_expression.push_str(&remove_fields.join(", "));
    }

    client
        .update_item()
        .table_name(table_name)
        .key("station", AttributeValue::S(station_name.to_string()))
        .key("chat_id", AttributeValue::N(chat_id.to_string()))
        .update_expression(update_expression)
        .set_expression_attribute_values(Some(expression_attribute_values))
        .send()
        .await?;

    Ok(())
}

pub async fn delete_alert(
    client: &Client,
    table_name: &str,
    station_name: &str,
    chat_id: i64,
) -> Result<bool> {
    if table_name.is_empty() {
        return Err(anyhow!("alerts table name is empty"));
    }

    let response = client
        .delete_item()
        .table_name(table_name)
        .key("station", AttributeValue::S(station_name.to_string()))
        .key("chat_id", AttributeValue::N(chat_id.to_string()))
        .return_values(aws_sdk_dynamodb::types::ReturnValue::AllOld)
        .send()
        .await?;

    Ok(response.attributes.is_some())
}

pub async fn alert_exists(
    client: &Client,
    table_name: &str,
    station_name: &str,
    chat_id: i64,
) -> Result<bool> {
    if table_name.is_empty() {
        return Err(anyhow!("alerts table name is empty"));
    }

    let response = client
        .get_item()
        .table_name(table_name)
        .key("station", AttributeValue::S(station_name.to_string()))
        .key("chat_id", AttributeValue::N(chat_id.to_string()))
        .send()
        .await?;

    Ok(response.item.is_some())
}

pub async fn list_active_alerts_for_chat(
    client: &Client,
    table_name: &str,
    chat_id: i64,
) -> Result<Vec<AlertEntry>> {
    if table_name.is_empty() {
        return Err(anyhow!("alerts table name is empty"));
    }

    let mut alerts = Vec::new();
    let mut last_evaluated_key = None;

    loop {
        let mut request = client
            .query()
            .table_name(table_name)
            .index_name("chat_id-active-index")
            .key_condition_expression("#chat_id = :chat_id AND #active = :active")
            .expression_attribute_names("#chat_id", "chat_id")
            .expression_attribute_names("#active", "active")
            .expression_attribute_values(":chat_id", AttributeValue::N(chat_id.to_string()))
            .expression_attribute_values(":active", AttributeValue::N(ALERT_ACTIVE.to_string()))
            .projection_expression("station, threshold, active, triggered_at, triggered_value");

        if let Some(key) = last_evaluated_key.take() {
            request = request.set_exclusive_start_key(Some(key));
        }

        let response = request.send().await?;
        for item in response.items.unwrap_or_default() {
            let station_name = parse_string_field(&item, "station")?;
            let threshold = parse_number_field::<f64>(&item, "threshold")?;
            let active = parse_number_field::<i64>(&item, "active")?;
            let triggered_at = parse_optional_number_field::<u64>(&item, "triggered_at")?;
            let triggered_value = parse_optional_number_field::<f64>(&item, "triggered_value")?;
            alerts.push(AlertEntry {
                station_name,
                threshold,
                active,
                triggered_at,
                triggered_value,
            });
        }

        if response.last_evaluated_key.is_none() {
            break;
        }
        last_evaluated_key = response.last_evaluated_key;
    }

    Ok(alerts)
}

pub async fn list_alerts_for_chat(
    client: &Client,
    table_name: &str,
    chat_id: i64,
) -> Result<Vec<AlertEntry>> {
    if table_name.is_empty() {
        return Err(anyhow!("alerts table name is empty"));
    }

    let mut alerts = Vec::new();

    for active_value in [ALERT_ACTIVE, ALERT_TRIGGERED] {
        let mut last_evaluated_key = None;
        loop {
            let mut request = client
                .query()
                .table_name(table_name)
                .index_name("chat_id-active-index")
                .key_condition_expression("#chat_id = :chat_id AND #active = :active")
                .expression_attribute_names("#chat_id", "chat_id")
                .expression_attribute_names("#active", "active")
                .expression_attribute_values(":chat_id", AttributeValue::N(chat_id.to_string()))
                .expression_attribute_values(":active", AttributeValue::N(active_value.to_string()))
                .projection_expression("station, threshold, active, triggered_at, triggered_value");

            if let Some(key) = last_evaluated_key.take() {
                request = request.set_exclusive_start_key(Some(key));
            }

            let response = request.send().await?;
            for item in response.items.unwrap_or_default() {
                let station_name = parse_string_field(&item, "station")?;
                let threshold = parse_number_field::<f64>(&item, "threshold")?;
                let active = parse_number_field::<i64>(&item, "active")?;
                let triggered_at = parse_optional_number_field::<u64>(&item, "triggered_at")?;
                let triggered_value = parse_optional_number_field::<f64>(&item, "triggered_value")?;
                alerts.push(AlertEntry {
                    station_name,
                    threshold,
                    active,
                    triggered_at,
                    triggered_value,
                });
            }

            if response.last_evaluated_key.is_none() {
                break;
            }
            last_evaluated_key = response.last_evaluated_key;
        }
    }

    Ok(alerts)
}

pub async fn count_active_alerts_for_chat(
    client: &Client,
    table_name: &str,
    chat_id: i64,
    max: usize,
) -> Result<usize> {
    if table_name.is_empty() {
        return Err(anyhow!("alerts table name is empty"));
    }

    let mut total = 0usize;
    let mut last_evaluated_key = None;

    loop {
        let mut request = client
            .query()
            .table_name(table_name)
            .index_name("chat_id-active-index")
            .key_condition_expression("#chat_id = :chat_id AND #active = :active")
            .expression_attribute_names("#chat_id", "chat_id")
            .expression_attribute_names("#active", "active")
            .expression_attribute_values(":chat_id", AttributeValue::N(chat_id.to_string()))
            .expression_attribute_values(":active", AttributeValue::N(ALERT_ACTIVE.to_string()))
            .select(aws_sdk_dynamodb::types::Select::Count)
            .limit((max.saturating_sub(total) + 1) as i32);

        if let Some(key) = last_evaluated_key.take() {
            request = request.set_exclusive_start_key(Some(key));
        }

        let response = request.send().await?;
        total += response.count() as usize;
        if total >= max || response.last_evaluated_key.is_none() {
            break;
        }
        last_evaluated_key = response.last_evaluated_key;
    }

    Ok(total)
}

pub async fn list_pending_alerts_for_station(
    client: &Client,
    table_name: &str,
    station_name: &str,
) -> Result<Vec<AlertSubscription>> {
    if table_name.is_empty() {
        return Err(anyhow!("alerts table name is empty"));
    }

    let response = client
        .query()
        .table_name(table_name)
        .index_name("station-active-index")
        .key_condition_expression("#station = :station AND #active = :active")
        .expression_attribute_names("#station", "station")
        .expression_attribute_names("#active", "active")
        .expression_attribute_values(":station", AttributeValue::S(station_name.to_string()))
        .expression_attribute_values(":active", AttributeValue::N(ALERT_ACTIVE.to_string()))
        .projection_expression("chat_id, threshold, thread_id")
        .send()
        .await?;

    let items = response.items.unwrap_or_default();
    let mut alerts = Vec::with_capacity(items.len());
    for item in items {
        let chat_id = parse_number_field::<i64>(&item, "chat_id")?;
        let threshold = parse_number_field::<f64>(&item, "threshold")?;
        let thread_id = parse_optional_number_field::<i64>(&item, "thread_id")?;

        alerts.push(AlertSubscription {
            chat_id,
            threshold,
            thread_id,
        });
    }

    Ok(alerts)
}

pub async fn reactivate_expired_alerts_for_station(
    client: &Client,
    table_name: &str,
    station_name: &str,
    now_millis: u64,
    cooldown_millis: u64,
) -> Result<usize> {
    if table_name.is_empty() {
        return Err(anyhow!("alerts table name is empty"));
    }

    let response = client
        .query()
        .table_name(table_name)
        .index_name("station-active-index")
        .key_condition_expression("#station = :station AND #active = :active")
        .expression_attribute_names("#station", "station")
        .expression_attribute_names("#active", "active")
        .expression_attribute_values(":station", AttributeValue::S(station_name.to_string()))
        .expression_attribute_values(":active", AttributeValue::N(ALERT_TRIGGERED.to_string()))
        .projection_expression("chat_id, triggered_at")
        .send()
        .await?;

    let items = response.items.unwrap_or_default();
    let mut reactivated = 0usize;
    for item in items {
        let chat_id = parse_number_field::<i64>(&item, "chat_id")?;
        let triggered_at = parse_optional_number_field::<u64>(&item, "triggered_at")?;
        let Some(triggered_at) = triggered_at else {
            continue;
        };
        if now_millis.saturating_sub(triggered_at) < cooldown_millis {
            continue;
        }

        let expression_attribute_values = HashMap::from([(
            ":active".to_string(),
            AttributeValue::N(ALERT_ACTIVE.to_string()),
        )]);

        client
            .update_item()
            .table_name(table_name)
            .key("station", AttributeValue::S(station_name.to_string()))
            .key("chat_id", AttributeValue::N(chat_id.to_string()))
            .update_expression("SET active = :active REMOVE triggered_at, triggered_value")
            .set_expression_attribute_values(Some(expression_attribute_values))
            .send()
            .await?;

        reactivated += 1;
    }

    Ok(reactivated)
}

pub async fn mark_alert_triggered(
    client: &Client,
    table_name: &str,
    station_name: &str,
    chat_id: i64,
    triggered_at: u64,
    value: f64,
) -> Result<()> {
    if table_name.is_empty() {
        return Err(anyhow!("alerts table name is empty"));
    }

    let expression_attribute_values = HashMap::from([
        (
            ":triggered_at".to_string(),
            AttributeValue::N(triggered_at.to_string()),
        ),
        (
            ":triggered_value".to_string(),
            AttributeValue::N(value.to_string()),
        ),
        (
            ":active".to_string(),
            AttributeValue::N(ALERT_TRIGGERED.to_string()),
        ),
    ]);

    client
        .update_item()
        .table_name(table_name)
        .key("station", AttributeValue::S(station_name.to_string()))
        .key("chat_id", AttributeValue::N(chat_id.to_string()))
        .update_expression(
            "SET triggered_at = :triggered_at, triggered_value = :triggered_value, active = :active",
        )
        .set_expression_attribute_values(Some(expression_attribute_values))
        .send()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn alert_entry_parses_from_item() {
        let item = HashMap::from([
            (
                "station".to_string(),
                AttributeValue::S("Cesena".to_string()),
            ),
            (
                "threshold".to_string(),
                AttributeValue::N("2.5".to_string()),
            ),
        ]);
        let station_name = parse_string_field(&item, "station").unwrap();
        let threshold = parse_number_field::<f64>(&item, "threshold").unwrap();
        let entry = AlertEntry {
            station_name,
            threshold,
            active: ALERT_ACTIVE.parse::<i64>().unwrap_or(1),
            triggered_at: None,
            triggered_value: None,
        };
        assert_eq!(entry.station_name, "Cesena");
        assert_eq!(entry.threshold, 2.5);
    }
}
