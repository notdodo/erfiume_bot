use crate::{logging, station::Station};
use anyhow::{Context, Result, anyhow};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use chrono::{DateTime, TimeZone};
use chrono_tz::Europe::Rome;
use erfiume_dynamodb::alerts::{
    AlertSubscription, list_pending_alerts_for_station, mark_alert_triggered,
    reactivate_expired_alerts_for_station,
};
use reqwest::Client as HTTPClient;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AlertsConfig {
    pub table_name: String,
    pub telegram_token: String,
}

impl AlertsConfig {
    pub fn from_env() -> Option<Self> {
        let table_name = std::env::var("ALERTS_TABLE_NAME").ok()?;
        if table_name.is_empty() {
            return None;
        }
        let telegram_token = std::env::var("TELOXIDE_TOKEN").ok()?;
        if telegram_token.is_empty() {
            return None;
        }
        Some(Self {
            table_name,
            telegram_token,
        })
    }
}

pub async fn process_alerts_for_station(
    http_client: &HTTPClient,
    dynamodb_client: &DynamoDbClient,
    station: &Station,
    config: &AlertsConfig,
) -> Result<()> {
    let Some(current_value) = station.value else {
        return Ok(());
    };

    let now_millis = current_time_millis();
    let cooldown_millis = 24 * 60 * 60 * 1000;
    let _ = reactivate_expired_alerts_for_station(
        dynamodb_client,
        &config.table_name,
        &station.nomestaz,
        now_millis,
        cooldown_millis,
    )
    .await;

    let pending_alerts =
        list_pending_alerts_for_station(dynamodb_client, &config.table_name, &station.nomestaz)
            .await
            .context("list_pending_alerts_for_station")?;

    if pending_alerts.is_empty() {
        return Ok(());
    }

    for alert in pending_alerts {
        if current_value < alert.threshold {
            continue;
        }

        if let Err(err) = send_alert(http_client, station, &alert, &config.telegram_token).await {
            let logger = logging::Logger::new()
                .station(&station.nomestaz)
                .chat_id(alert.chat_id);
            logger.error("alerts.send_failed", &err, "Failed to send alert");
            continue;
        }

        let triggered_at = station.timestamp.unwrap_or(now_millis);

        if let Err(err) = mark_alert_triggered(
            dynamodb_client,
            &config.table_name,
            &station.nomestaz,
            alert.chat_id,
            triggered_at,
            current_value,
        )
        .await
        .context("mark_alert_triggered")
        {
            let logger = logging::Logger::new()
                .station(&station.nomestaz)
                .chat_id(alert.chat_id);
            logger.error(
                "alerts.mark_triggered_failed",
                &err,
                "Failed to mark alert as triggered",
            );
            continue;
        }

        let logger = logging::Logger::new()
            .station(&station.nomestaz)
            .chat_id(alert.chat_id)
            .threshold(alert.threshold)
            .value(current_value);
        logger.info("alerts.triggered", "Alert triggered");
    }

    Ok(())
}

async fn send_alert(
    http_client: &HTTPClient,
    station: &Station,
    alert: &AlertSubscription,
    telegram_token: &str,
) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{telegram_token}/sendMessage");
    let message = format!(
        "Avviso soglia: {} ha raggiunto {} (soglia {}).\n\n{}",
        station.nomestaz,
        station.value.unwrap_or_default(),
        alert.threshold,
        format_station_message(station)
    );

    let mut payload = json!({
        "chat_id": alert.chat_id,
        "text": message,
    });
    if let Some(thread_id) = alert.thread_id {
        payload["message_thread_id"] = json!(thread_id);
    }

    let response = http_client.post(url).json(&payload).send().await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "telegram api error: status={} body={}",
            status,
            body
        ));
    }

    Ok(())
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn format_station_message(station: &Station) -> String {
    const UNKNOWN_VALUE: f32 = -9999.0;
    let timestamp_formatted = station
        .timestamp
        .and_then(|timestamp| {
            let timestamp_secs = (timestamp / 1000) as i64;
            let naive_datetime = DateTime::from_timestamp(timestamp_secs, 0)?;
            let datetime_in_tz = Rome.from_utc_datetime(&naive_datetime.naive_utc());
            Some(datetime_in_tz.format("%d-%m-%Y %H:%M").to_string())
        })
        .unwrap_or_else(|| "non disponibile".to_string());

    let value = station.value.unwrap_or(UNKNOWN_VALUE);
    let yellow = station.soglia1;
    let orange = station.soglia2;
    let red = station.soglia3;

    let mut alarm = "🔴";
    if value <= yellow {
        alarm = "🟢";
    } else if value > yellow && value <= orange {
        alarm = "🟡";
    } else if value >= orange && value <= red {
        alarm = "🟠";
    }

    let mut value_str = format!("{value:.2}");
    if value == UNKNOWN_VALUE {
        value_str = "non disponibile".to_string();
        alarm = "";
    }

    let yellow_str = format!("{yellow:.2}");
    let orange_str = format!("{orange:.2}");
    let red_str = format!("{red:.2}");

    format!(
        "Stazione: {}\nValore: {} {}\nSoglia Gialla: {}\nSoglia Arancione: {}\nSoglia Rossa: {}\nUltimo rilevamento: {}",
        station.nomestaz, value_str, alarm, yellow_str, orange_str, red_str, timestamp_formatted
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_station_message_matches_bot_format() {
        let station = Station {
            timestamp: Some(1766848325000),
            idstazione: "id".to_string(),
            ordinamento: 1,
            nomestaz: "Cesena".to_string(),
            lon: "lon".to_string(),
            lat: "lat".to_string(),
            soglia1: 1.0,
            soglia2: 2.0,
            soglia3: 3.0,
            value: Some(2.2),
        };

        let expected = "Stazione: Cesena\nValore: 2.20 🟠\nSoglia Gialla: 1.00\nSoglia Arancione: 2.00\nSoglia Rossa: 3.00\nUltimo rilevamento: 27-12-2025 16:12";
        assert_eq!(format_station_message(&station), expected);
    }
}
