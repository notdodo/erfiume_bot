use super::commands::CommandHandler;
use super::parsing::{parse_station_arg, parse_station_threshold_args};
use super::regions::{ensure_region_selected, stations_scan_page_size};
use crate::commands::utils::format_alert_status;
use crate::station;
use chrono::Utc;
use erfiume_dynamodb::alerts as dynamo_alerts;
use erfiume_dynamodb::utils::current_time_millis;

pub(super) async fn handle_lista_avvisi(
    handler: &CommandHandler<'_>,
) -> Result<(), teloxide::RequestError> {
    let Some(alerts_table_name) = alerts_table_name() else {
        return handler
            .send_text("Funzionalità non disponibile al momento.")
            .await;
    };

    let chat_id = handler.msg().chat.id.0;
    let alerts =
        match dynamo_alerts::list_alerts_for_chat(handler.dynamodb(), &alerts_table_name, chat_id)
            .await
        {
            Ok(alerts) => alerts,
            Err(err) => {
                handler.logger().clone().table(&alerts_table_name).error(
                    "alerts.list_failed",
                    &err,
                    "Failed to list alerts",
                );
                handler
                    .send_text("Errore nel recupero degli avvisi. Riprova più tardi.")
                    .await?;
                return Ok(());
            }
        };

    let text = if alerts.is_empty() {
        "Non hai avvisi attivi.".to_string()
    } else {
        let now_millis = current_time_millis();
        let mut lines = Vec::with_capacity(alerts.len() + 1);
        lines.push("I tuoi avvisi:".to_string());
        for (index, alert) in alerts.iter().enumerate() {
            let status = format_alert_status(alert, now_millis);
            lines.push(format!(
                "{}. {} - {} ({})",
                index + 1,
                alert.station_name,
                alert.threshold,
                status
            ));
        }
        lines.join("\n")
    };

    handler.send_text(&text).await
}

pub(super) async fn handle_rimuovi_avviso(
    handler: &CommandHandler<'_>,
    args: String,
) -> Result<(), teloxide::RequestError> {
    let Some(station_name) = parse_station_arg(args) else {
        handler
            .send_text("Uso: /rimuovi_avviso <stazione> oppure /rimuovi_avviso <numero>")
            .await?;
        return Ok(());
    };

    let Some(alerts_table_name) = alerts_table_name() else {
        return handler
            .send_text("Funzionalità non disponibile al momento.")
            .await;
    };

    let chat_id = handler.msg().chat.id.0;
    if let Ok(index) = station_name.parse::<usize>() {
        let alerts = match dynamo_alerts::list_alerts_for_chat(
            handler.dynamodb(),
            &alerts_table_name,
            chat_id,
        )
        .await
        {
            Ok(alerts) => alerts,
            Err(err) => {
                handler.logger().clone().table(&alerts_table_name).error(
                    "alerts.list_failed",
                    &err,
                    "Failed to list alerts",
                );
                handler
                    .send_text("Errore nel recupero degli avvisi. Riprova più tardi.")
                    .await?;
                return Ok(());
            }
        };

        if index == 0 || index > alerts.len() {
            handler
                .send_text("Numero non valido. Usa /lista_avvisi per vedere gli avvisi attivi.")
                .await?;
            return Ok(());
        }

        let alert = &alerts[index - 1];
        let removed = match dynamo_alerts::delete_alert(
            handler.dynamodb(),
            &alerts_table_name,
            &alert.station_name,
            chat_id,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                handler
                    .logger()
                    .clone()
                    .station(&alert.station_name)
                    .table(&alerts_table_name)
                    .error("alerts.delete_failed", &err, "Failed to delete alert");
                false
            }
        };

        let text = if removed {
            format!("Avviso rimosso per {}.", alert.station_name)
        } else {
            "Non ho trovato un avviso attivo per questa stazione.".to_string()
        };
        return handler.send_text(&text).await;
    }

    let Some(stations_table_name) = ensure_region_selected(
        handler.ctx(),
        handler.bot(),
        handler.msg(),
        handler.link_preview_options(),
    )
    .await?
    else {
        return Ok(());
    };

    let scan_page_size = stations_scan_page_size();
    let station_result = station::search::get_station_with_match(
        handler.dynamodb(),
        station_name,
        stations_table_name.as_str(),
        scan_page_size,
    )
    .await
    .map(|result| result.map(|(station, _)| station));

    let station = match station_result {
        Ok(Some(item)) => item,
        _ => {
            handler
                .send_text(
                    "Nessuna stazione trovata con quel nome. Usa /stazioni per vedere l'elenco.",
                )
                .await?;
            return Ok(());
        }
    };

    let removed = match dynamo_alerts::delete_alert(
        handler.dynamodb(),
        &alerts_table_name,
        &station.nomestaz,
        chat_id,
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            handler
                .logger()
                .clone()
                .station(&station.nomestaz)
                .table(&alerts_table_name)
                .error("alerts.delete_failed", &err, "Failed to delete alert");
            false
        }
    };

    let text = if removed {
        format!("Avviso rimosso per {}.", station.nomestaz)
    } else {
        "Non ho trovato un avviso attivo per questa stazione.".to_string()
    };
    handler.send_text(&text).await
}

pub(super) async fn handle_avvisami(
    handler: &CommandHandler<'_>,
    args: String,
) -> Result<(), teloxide::RequestError> {
    let Some((station_name, threshold)) = parse_station_threshold_args(args) else {
        handler
            .send_text("Uso: /avvisami <stazione> <valoreSoglia>")
            .await?;
        return Ok(());
    };

    let Some(alerts_table_name) = alerts_table_name() else {
        return handler
            .send_text("Funzionalità non disponibile al momento.")
            .await;
    };

    let chat_id = handler.msg().chat.id.0;
    let thread_id = handler.msg().thread_id.map(|id| i64::from(id.0.0));
    let Some(stations_table_name) = ensure_region_selected(
        handler.ctx(),
        handler.bot(),
        handler.msg(),
        handler.link_preview_options(),
    )
    .await?
    else {
        return Ok(());
    };

    let scan_page_size = stations_scan_page_size();
    let station_result = station::search::get_station_with_match(
        handler.dynamodb(),
        station_name,
        stations_table_name.as_str(),
        scan_page_size,
    )
    .await
    .map(|result| result.map(|(station, _)| station));

    let station = match station_result {
        Ok(Some(item)) => item,
        _ => {
            handler
                .send_text(
                    "Nessuna stazione trovata con quel nome. Usa /stazioni per vedere l'elenco.",
                )
                .await?;
            return Ok(());
        }
    };

    let already_exists = match dynamo_alerts::alert_exists(
        handler.dynamodb(),
        &alerts_table_name,
        &station.nomestaz,
        chat_id,
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            handler.logger().clone().station(&station.nomestaz).error(
                "alerts.exists_check_failed",
                &err,
                "Failed to check alert existence",
            );
            handler
                .send_text("Errore nel salvataggio dell'avviso. Riprova più tardi.")
                .await?;
            return Ok(());
        }
    };

    if !already_exists {
        let count = match dynamo_alerts::count_active_alerts_for_chat(
            handler.dynamodb(),
            &alerts_table_name,
            chat_id,
            3,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                handler.logger().clone().table(&alerts_table_name).error(
                    "alerts.count_failed",
                    &err,
                    "Failed to count alerts",
                );
                handler
                    .send_text("Errore nel salvataggio dell'avviso. Riprova più tardi.")
                    .await?;
                return Ok(());
            }
        };
        if count >= 3 {
            handler
                .send_text("Hai già impostato 3 avvisi. Per evitare spam, il limite è 3.")
                .await?;
            return Ok(());
        }
    }

    let created_at = Utc::now().timestamp();
    if let Err(err) = dynamo_alerts::upsert_alert(
        handler.dynamodb(),
        &alerts_table_name,
        &station.nomestaz,
        chat_id,
        threshold,
        created_at,
        thread_id,
    )
    .await
    {
        handler
            .logger()
            .clone()
            .station(&station.nomestaz)
            .table(&alerts_table_name)
            .error("alerts.save_failed", &err, "Failed to save alert");
        handler
            .send_text("Errore nel salvataggio dell'avviso. Riprova più tardi.")
            .await?;
        return Ok(());
    }

    handler
        .send_text(&format!(
            "Ok! Ti avviser• quando {} supera {}.",
            station.nomestaz, threshold
        ))
        .await
}

fn alerts_table_name() -> Option<String> {
    let raw = std::env::var("ALERTS_TABLE_NAME").unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
