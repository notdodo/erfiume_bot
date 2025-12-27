use crate::station::Station;
use anyhow::{Context, Result, anyhow};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use erfiume_dynamodb::alerts::{
    AlertSubscription, list_pending_alerts_for_station, mark_alert_triggered,
    reactivate_expired_alerts_for_station,
};
use reqwest::Client as HTTPClient;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};

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
            error!(
                station = %station.nomestaz,
                chat_id = alert.chat_id,
                error = ?err,
                "Failed to send alert"
            );
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
            error!(
                station = %station.nomestaz,
                chat_id = alert.chat_id,
                error = ?err,
                "Failed to mark alert as triggered"
            );
            continue;
        }

        info!(
            station = %station.nomestaz,
            chat_id = alert.chat_id,
            threshold = alert.threshold,
            value = current_value,
            "Alert triggered"
        );
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
        "Avviso soglia: {} ha raggiunto {} (soglia {}).",
        station.nomestaz,
        station.value.unwrap_or_default(),
        alert.threshold
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
