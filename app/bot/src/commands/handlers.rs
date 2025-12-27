use super::{Command, utils};
use crate::station;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use chrono::Utc;
use erfiume_dynamodb::ALERT_ACTIVE;
use erfiume_dynamodb::alerts as dynamo_alerts;
use erfiume_dynamodb::chats as dynamo_chats;
use std::time::{SystemTime, UNIX_EPOCH};
use teloxide::{
    prelude::Bot,
    types::{LinkPreviewOptions, Message},
    utils::command::BotCommands,
};
use tracing::error;

pub(crate) async fn commands_handler(
    bot: Bot,
    msg: Message,
    cmd: Command,
    dynamodb_client: DynamoDbClient,
) -> Result<(), teloxide::RequestError> {
    let link_preview_options = link_preview_disabled();
    // Move it to /start only after a while to collect old users
    ensure_chat_presence(&dynamodb_client, &msg).await;

    let text = match cmd {
        Command::Help => Command::descriptions().to_string(),
        Command::Start => {
            if msg.chat.is_group() || msg.chat.is_supergroup() {
                format!(
                    "Ciao {}! Scrivete il nome di una stazione da monitorare (e.g. /Cesena@erfiume_bot o /Borello@erfiume_bot) \
                        o cercatene una con /stazioni@erfiume_bot",
                    msg.chat.title().unwrap_or("")
                )
            } else {
                format!(
                    "Ciao @{}! Scrivi il nome di una stazione da monitorare (e.g. `Cesena` o /SCarlo) \
                        o cercane una con /stazioni",
                    msg.chat
                        .username()
                        .unwrap_or(msg.chat.first_name().unwrap_or(""))
                )
            }
        }
        Command::Stazioni => station::stations().join("\n"),
        Command::Info => {
            let info = "Bot Telegram che permette di leggere i livelli idrometrici dei fiumi dell'Emilia-Romagna \
                                I dati idrometrici sono ottenuti dalle API messe a disposizione da allertameteo.regione.emilia-romagna.it\n\n\
                                Il progetto è completamente open-source (https://github.com/notdodo/erfiume_bot).\n\
                                Per sostenere e mantenere il servizio attivo: buymeacoffee.com/d0d0\n\n\
                                Inizia con /start o /stazioni";
            info.to_string()
        }
        Command::ListaAvvisi => {
            let alerts_table_name = std::env::var("ALERTS_TABLE_NAME").unwrap_or_default();
            if alerts_table_name.is_empty() {
                "Funzionalità non disponibile al momento.".to_string()
            } else {
                let chat_id = msg.chat.id.0;
                let alerts = match dynamo_alerts::list_alerts_for_chat(
                    &dynamodb_client,
                    &alerts_table_name,
                    chat_id,
                )
                .await
                {
                    Ok(alerts) => alerts,
                    Err(err) => {
                        error!(
                            target: "erfiume_bot",
                            error = ?err,
                            chat_id,
                            table = %alerts_table_name,
                            "Failed to list alerts"
                        );
                        utils::send_message(
                            &bot,
                            &msg,
                            link_preview_options,
                            "Errore nel recupero degli avvisi. Riprova più tardi.",
                        )
                        .await?;
                        return Ok(());
                    }
                };

                if alerts.is_empty() {
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
                }
            }
        }
        Command::Rimuoviavviso(args) | Command::Rimuoviavvisi(args) => {
            let Some(station_name) = parse_station_arg(args) else {
                utils::send_message(
                    &bot,
                    &msg,
                    link_preview_options,
                    "Uso: /rimuovi_avviso <stazione> oppure /rimuovi_avviso <numero>",
                )
                .await?;
                return Ok(());
            };

            let alerts_table_name = std::env::var("ALERTS_TABLE_NAME").unwrap_or_default();
            if alerts_table_name.is_empty() {
                "Funzionalità non disponibile al momento.".to_string()
            } else {
                let chat_id = msg.chat.id.0;
                if let Ok(index) = station_name.parse::<usize>() {
                    let alerts = match dynamo_alerts::list_alerts_for_chat(
                        &dynamodb_client,
                        &alerts_table_name,
                        chat_id,
                    )
                    .await
                    {
                        Ok(alerts) => alerts,
                        Err(err) => {
                            error!(
                                target: "erfiume_bot",
                                error = ?err,
                                chat_id,
                                table = %alerts_table_name,
                                "Failed to list alerts"
                            );
                            utils::send_message(
                                &bot,
                                &msg,
                                link_preview_options,
                                "Errore nel recupero degli avvisi. Riprova più tardi.",
                            )
                            .await?;
                            return Ok(());
                        }
                    };

                    if index == 0 || index > alerts.len() {
                        utils::send_message(
                            &bot,
                            &msg,
                            link_preview_options,
                            "Numero non valido. Usa /lista_avvisi per vedere gli avvisi attivi.",
                        )
                        .await?;
                        return Ok(());
                    }

                    let alert = &alerts[index - 1];
                    let removed = match dynamo_alerts::delete_alert(
                        &dynamodb_client,
                        &alerts_table_name,
                        &alert.station_name,
                        chat_id,
                    )
                    .await
                    {
                        Ok(value) => value,
                        Err(err) => {
                            error!(
                                target: "erfiume_bot",
                                error = ?err,
                                station = %alert.station_name,
                                chat_id,
                                table = %alerts_table_name,
                                "Failed to delete alert"
                            );
                            false
                        }
                    };

                    if removed {
                        format!("Avviso rimosso per {}.", alert.station_name)
                    } else {
                        "Non ho trovato un avviso attivo per questa stazione.".to_string()
                    }
                } else {
                    let station_result = station::search::get_station(
                        &dynamodb_client,
                        station_name,
                        "EmiliaRomagna-Stations",
                    )
                    .await;

                    let station = match station_result {
                        Ok(Some(item)) => item,
                        _ => {
                            utils::send_message(
                                &bot,
                                &msg,
                                link_preview_options,
                                "Nessuna stazione trovata con quel nome. Usa /stazioni per vedere l'elenco.",
                            )
                            .await?;
                            return Ok(());
                        }
                    };

                    let removed = match dynamo_alerts::delete_alert(
                        &dynamodb_client,
                        &alerts_table_name,
                        &station.nomestaz,
                        chat_id,
                    )
                    .await
                    {
                        Ok(value) => value,
                        Err(err) => {
                            error!(
                                target: "erfiume_bot",
                                error = ?err,
                                station = %station.nomestaz,
                                chat_id,
                                table = %alerts_table_name,
                                "Failed to delete alert"
                            );
                            false
                        }
                    };

                    if removed {
                        format!("Avviso rimosso per {}.", station.nomestaz)
                    } else {
                        "Non ho trovato un avviso attivo per questa stazione.".to_string()
                    }
                }
            }
        }
        Command::Avvisami(args) => {
            let Some((station_name, threshold)) = parse_station_threshold_args(args) else {
                utils::send_message(
                    &bot,
                    &msg,
                    link_preview_options,
                    "Uso: /avvisami <stazione> <valoreSoglia>",
                )
                .await?;
                return Ok(());
            };

            let alerts_table_name = std::env::var("ALERTS_TABLE_NAME").unwrap_or_default();
            if alerts_table_name.is_empty() {
                "Funzionalità non disponibile al momento.".to_string()
            } else {
                let chat_id = msg.chat.id.0;
                let thread_id = msg.thread_id.map(|id| i64::from(id.0.0));
                let station_result = station::search::get_station(
                    &dynamodb_client,
                    station_name,
                    "EmiliaRomagna-Stations",
                )
                .await;

                let station = match station_result {
                    Ok(Some(item)) => item,
                    _ => {
                        utils::send_message(
                            &bot,
                            &msg,
                            link_preview_options,
                            "Nessuna stazione trovata con quel nome. Usa /stazioni per vedere l'elenco.",
                        )
                        .await?;
                        return Ok(());
                    }
                };

                let already_exists = match dynamo_alerts::alert_exists(
                    &dynamodb_client,
                    &alerts_table_name,
                    &station.nomestaz,
                    chat_id,
                )
                .await
                {
                    Ok(value) => value,
                    Err(err) => {
                        error!(
                            target: "erfiume_bot",
                            error = ?err,
                            "Failed to check alert existence command=avvisami chat_id={} stations={}",
                            chat_id,
                            station.nomestaz,
                        );
                        utils::send_message(
                            &bot,
                            &msg,
                            link_preview_options,
                            "Errore nel salvataggio dell'avviso. Riprova pi— tardi.",
                        )
                        .await?;
                        return Ok(());
                    }
                };
                if !already_exists {
                    let count = match dynamo_alerts::count_active_alerts_for_chat(
                        &dynamodb_client,
                        &alerts_table_name,
                        chat_id,
                        3,
                    )
                    .await
                    {
                        Ok(value) => value,
                        Err(err) => {
                            error!(
                                target: "erfiume_bot",
                                error = ?err,
                                "Failed to count alerts command=avvisami chat_id={} stations={}",
                                chat_id,
                                alerts_table_name,
                            );
                            utils::send_message(
                                &bot,
                                &msg,
                                link_preview_options,
                                "Errore nel salvataggio dell'avviso. Riprova pi— tardi.",
                            )
                            .await?;
                            return Ok(());
                        }
                    };
                    if count >= 3 {
                        utils::send_message(
                            &bot,
                            &msg,
                            link_preview_options,
                            "Hai già impostato 3 avvisi. Per evitare spam, il limite è 3.",
                        )
                        .await?;
                        return Ok(());
                    }
                }

                let created_at = Utc::now().timestamp();
                if let Err(err) = dynamo_alerts::upsert_alert(
                    &dynamodb_client,
                    &alerts_table_name,
                    &station.nomestaz,
                    chat_id,
                    threshold,
                    created_at,
                    thread_id,
                )
                .await
                {
                    error!(
                        target: "erfiume_bot",
                        error = ?err,
                        "Failed to save alert command=avvisami chat_id={} stations={} table={}",
                        chat_id,
                        station.nomestaz,
                        alerts_table_name,
                    );
                    utils::send_message(
                        &bot,
                        &msg,
                        link_preview_options,
                        "Errore nel salvataggio dell'avviso. Riprova più tardi.",
                    )
                    .await?;
                    return Ok(());
                }

                format!(
                    "Ok! Ti avviserò quando {} supera {}.",
                    station.nomestaz, threshold
                )
            }
        }
    };

    utils::send_message(&bot, &msg, link_preview_options, &text).await?;
    Ok(())
}

pub(crate) async fn message_handler(
    bot: &Bot,
    msg: &Message,
    dynamodb_client: &DynamoDbClient,
) -> Result<(), teloxide::RequestError> {
    let link_preview_options = link_preview_small_media();
    // Move it to /start only after a while to collect old users
    ensure_chat_presence(dynamodb_client, msg).await;
    let Some(text) = msg.text() else {
        return Ok(());
    };

    let text = match station::search::get_station(
        dynamodb_client,
        text.trim().replace("@erfiume_bot", "").replace("/", ""),
        "EmiliaRomagna-Stations",
    )
    .await
    {
        Ok(Some(item)) => {
            if item.nomestaz.to_lowercase() != text.to_lowercase() {
                format!(
                    "{}\nSe non è la stazione corretta prova ad affinare la ricerca.",
                    item.create_station_message()
                )
            } else {
                item.create_station_message().to_string()
            }
        }
        Err(_) | Ok(None) => "Nessuna stazione trovata con la parola di ricerca.\nInserisci esattamente il nome che vedi nella pagina https://allertameteo.regione.emilia-romagna.it/livello-idrometrico\nAd esempio 'Cesena', 'Lavino di Sopra' o 'S. Carlo'.\nSe non sai quale cercare, prova con /stazioni.".to_string(),
    };

    let mut message = text.clone();
    if fastrand::usize(0..10) == 8 {
        message = format!(
            "{text}\n\nContribuisci al progetto per mantenerlo attivo e sviluppare nuove funzionalità tramite una donazione: https://buymeacoffee.com/d0d0",
        );
    }
    if fastrand::usize(0..50) == 8 {
        message = format!(
            "{text}\n\nEsplora o contribuisci al progetto open-source per sviluppare nuove funzionalità: https://github.com/notdodo/erfiume_bot"
        );
    }
    utils::send_message(bot, msg, link_preview_options, &message).await?;

    Ok(())
}

async fn ensure_chat_presence(dynamodb_client: &DynamoDbClient, msg: &Message) {
    let chats_table_name = std::env::var("CHATS_TABLE_NAME").unwrap_or_default();
    if chats_table_name.is_empty() {
        return;
    }

    let chat_type = if msg.chat.is_private() {
        "private"
    } else if msg.chat.is_group() {
        "group"
    } else if msg.chat.is_supergroup() {
        "supergroup"
    } else if msg.chat.is_channel() {
        "channel"
    } else {
        "other"
    };

    let record = dynamo_chats::ChatRecord {
        chat_id: msg.chat.id.0,
        chat_type: chat_type.to_string(),
        username: msg.chat.username().map(|value| value.to_string()),
        first_name: msg.chat.first_name().map(|value| value.to_string()),
        last_name: msg.chat.last_name().map(|value| value.to_string()),
        title: msg.chat.title().map(|value| value.to_string()),
        created_at: Utc::now().timestamp(),
    };

    if let Err(err) =
        dynamo_chats::insert_chat_if_missing(dynamodb_client, &chats_table_name, &record).await
    {
        error!(
            target: "erfiume_bot",
            error = ?err,
            "Failed to store chat chat_id={} table={}",
            record.chat_id,
            chats_table_name,
        );
    }
}

fn parse_station_arg(arg: String) -> Option<String> {
    let station_name = arg.trim().to_string();
    (!station_name.is_empty()).then_some(station_name)
}

fn parse_station_threshold_args(arg: String) -> Option<(String, f64)> {
    let raw = arg;
    let mut parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let threshold_raw = parts.pop()?.replace(',', ".");
    let threshold = threshold_raw.parse::<f64>().ok()?;
    let station_name = parts.join(" ").trim().to_string();
    (!station_name.is_empty()).then_some((station_name, threshold))
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn format_alert_status(alert: &dynamo_alerts::AlertEntry, now_millis: u64) -> String {
    const COOLDOWN_MILLIS: u64 = 24 * 60 * 60 * 1000;
    if alert.active == ALERT_ACTIVE.parse::<i64>().unwrap_or(1) {
        return "attivo".to_string();
    }

    let remaining = alert.triggered_at.map(|triggered_at| {
        COOLDOWN_MILLIS.saturating_sub(now_millis.saturating_sub(triggered_at))
    });

    let mut status = if let Some(value) = alert.triggered_value {
        format!("in pausa (soglia superata: {})", value)
    } else {
        "in pausa (soglia superata)".to_string()
    };

    if let Some(remaining) = remaining {
        if remaining == 0 {
            status.push_str(", ripristino imminente");
        } else {
            status.push_str(", ripristino tra ");
            status.push_str(&format_duration_millis(remaining));
        }
    } else {
        status.push_str(", ripristino in attesa");
    }

    status
}

fn format_duration_millis(millis: u64) -> String {
    let total_secs = millis.div_ceil(1000);
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m", minutes)
    } else {
        format!("{}s", seconds)
    }
}

fn link_preview_disabled() -> LinkPreviewOptions {
    LinkPreviewOptions {
        is_disabled: true,
        url: None,
        prefer_small_media: false,
        prefer_large_media: false,
        show_above_text: false,
    }
}

fn link_preview_small_media() -> LinkPreviewOptions {
    LinkPreviewOptions {
        is_disabled: false,
        url: None,
        prefer_small_media: true,
        prefer_large_media: false,
        show_above_text: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_station_arg_rejects_blank() {
        assert_eq!(parse_station_arg("   ".to_string()), None);
    }

    #[test]
    fn parse_station_threshold_args_parses_station_and_threshold() {
        let parsed = parse_station_threshold_args("S. Carlo 2,5".to_string());
        assert_eq!(parsed, Some(("S. Carlo".to_string(), 2.5)));
    }

    #[test]
    fn parse_station_threshold_args_rejects_missing_threshold() {
        assert_eq!(parse_station_threshold_args("Cesena".to_string()), None);
    }

    #[test]
    fn format_duration_millis_prefers_hours_and_minutes() {
        assert_eq!(format_duration_millis(3_600_000), "1h 0m");
        assert_eq!(format_duration_millis(60_000), "1m");
        assert_eq!(format_duration_millis(500), "1s");
    }

    #[test]
    fn format_alert_status_active() {
        let alert = dynamo_alerts::AlertEntry {
            station_name: "Cesena".to_string(),
            threshold: 1.0,
            active: ALERT_ACTIVE.parse::<i64>().unwrap_or(1),
            triggered_at: None,
            triggered_value: None,
        };
        assert_eq!(format_alert_status(&alert, 0), "attivo");
    }

    #[test]
    fn format_alert_status_triggered_imminent() {
        let alert = dynamo_alerts::AlertEntry {
            station_name: "Cesena".to_string(),
            threshold: 1.0,
            active: 0,
            triggered_at: Some(0),
            triggered_value: Some(2.5),
        };
        let status = format_alert_status(&alert, 24 * 60 * 60 * 1000);
        assert_eq!(
            status,
            "in pausa (soglia superata: 2.5), ripristino imminente"
        );
    }
}
