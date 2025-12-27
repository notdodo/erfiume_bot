use aws_sdk_dynamodb::Client as DynamoDbClient;
use chrono::Utc;
use erfiume_dynamodb::alerts as dynamo_alerts;
use teloxide::{
    prelude::Bot,
    types::{LinkPreviewOptions, Message},
    utils::command::BotCommands,
};
use tracing::{error, info};

use crate::station;
pub(crate) mod utils;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub(crate) enum Command {
    /// Visualizza la lista dei comandi
    Help,
    /// Ottieni informazioni riguardanti il bot
    Info,
    /// Inizia ad interagire con il bot
    Start,
    /// Visualizza la lista delle stazioni disponibili
    Stazioni,
    /// Ricevi un avviso quando la soglia viene superata
    Avvisami(String),
    /// Lista dei tuoi avvisi attivi
    #[command(rename = "lista_avvisi")]
    ListaAvvisi,
    /// Rimuovi un avviso per la stazione
    #[command(rename = "rimuovi_avviso")]
    Rimuoviavviso(String),
    /// Rimuovi un avviso per la stazione (alias)
    #[command(rename = "rimuovi_avvisi")]
    Rimuoviavvisi(String),
}

pub(crate) async fn commands_handler(
    bot: Bot,
    msg: Message,
    cmd: Command,
    dynamodb_client: DynamoDbClient,
) -> Result<(), teloxide::RequestError> {
    let link_preview_options = LinkPreviewOptions {
        is_disabled: true,
        url: None,
        prefer_small_media: false,
        prefer_large_media: false,
        show_above_text: false,
    };

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
                let alerts = match dynamo_alerts::list_active_alerts_for_chat(
                    &dynamodb_client,
                    &alerts_table_name,
                    chat_id,
                )
                .await
                {
                    Ok(alerts) => alerts,
                    Err(err) => {
                        error!("Failed to list alerts: chat_id={} error={}", chat_id, err);
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
                    let mut lines = Vec::with_capacity(alerts.len() + 1);
                    lines.push("I tuoi avvisi attivi:".to_string());
                    for (index, alert) in alerts.iter().enumerate() {
                        lines.push(format!(
                            "{}. {} - {}",
                            index + 1,
                            alert.station_name,
                            alert.threshold
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
                    let alerts = match dynamo_alerts::list_active_alerts_for_chat(
                        &dynamodb_client,
                        &alerts_table_name,
                        chat_id,
                    )
                    .await
                    {
                        Ok(alerts) => alerts,
                        Err(err) => {
                            error!("Failed to list alerts: chat_id={} error={}", chat_id, err);
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
                    let removed = dynamo_alerts::delete_alert(
                        &dynamodb_client,
                        &alerts_table_name,
                        &alert.station_name,
                        chat_id,
                    )
                    .await
                    .unwrap_or(false);

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

                    let removed = dynamo_alerts::delete_alert(
                        &dynamodb_client,
                        &alerts_table_name,
                        &station.nomestaz,
                        chat_id,
                    )
                    .await
                    .unwrap_or(false);

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
                info!(
                    target: "erfiume_bot",
                    "command=avvisami chat_id={} thread_id={:?} table={}",
                    chat_id,
                    thread_id,
                    alerts_table_name
                );

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
                            "Failed to check alert existence: station={} chat_id={} error={}",
                            station.nomestaz, chat_id, err
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
                };

                info!(
                    target: "erfiume_bot",
                    "command=avvisami chat_id={} station={} exists={}",
                    chat_id,
                    station.nomestaz,
                    already_exists
                );

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
                            error!("Failed to count alerts: chat_id={} error={}", chat_id, err);
                            utils::send_message(
                                &bot,
                                &msg,
                                link_preview_options,
                                "Errore nel salvataggio dell'avviso. Riprova più tardi.",
                            )
                            .await?;
                            return Ok(());
                        }
                    };

                    info!(
                        target: "erfiume_bot",
                        "command=avvisami chat_id={} active_count={}",
                        chat_id,
                        count
                    );

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
                        "Failed to save alert: station={} chat_id={} error={}",
                        station.nomestaz, chat_id, err
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
    let link_preview_options = LinkPreviewOptions {
        is_disabled: false,
        url: None,
        prefer_small_media: true,
        prefer_large_media: false,
        show_above_text: false,
    };

    let Some(text) = msg.text() else {
        return Ok(());
    };

    let trimmed_text = text.trim();
    if trimmed_text.starts_with('/') {
        return Ok(());
    }

    let text = match station::search::get_station(
        dynamodb_client,
        text.to_string().replace("@erfiume_bot", "").replace("/", ""),
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
    if fastrand::choose_multiple(0..10, 1)[0] == 8 {
        message = format!(
            "{text}\n\nContribuisci al progetto per mantenerlo attivo e sviluppare nuove funzionalità tramite una donazione: https://buymeacoffee.com/d0d0",
        );
    }
    if fastrand::choose_multiple(0..50, 1)[0] == 8 {
        message = format!(
            "{text}\n\nEsplora o contribuisci al progetto open-source per sviluppare nuove funzionalità: https://github.com/notdodo/erfiume_bot"
        );
    }
    utils::send_message(bot, msg, link_preview_options, &message).await?;

    Ok(())
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
