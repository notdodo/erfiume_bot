use super::{Command, utils};
use crate::commands::utils::{
    current_time_millis, format_alert_status, link_preview_disabled, link_preview_small_media,
};
use crate::{logging, station};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use chrono::Utc;
use erfiume_dynamodb::alerts as dynamo_alerts;
use erfiume_dynamodb::chats as dynamo_chats;
use std::sync::OnceLock;
use teloxide::{
    payloads::{AnswerCallbackQuerySetters, EditMessageTextSetters, SendMessageSetters},
    prelude::{Bot, Requester},
    types::{
        CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup, LinkPreviewOptions,
        MaybeInaccessibleMessage, Message, ParseMode,
    },
    utils::command::BotCommands,
};

const REGION_CALLBACK_PREFIX: &str = "region:";
const DEFAULT_SCAN_PAGE_SIZE: i32 = 25;
const MAX_SCAN_PAGE_SIZE: i32 = 100;

struct RegionConfig {
    key: String,
    label: String,
    table_name: String,
}

struct RegionsConfig {
    emilia_romagna: RegionConfig,
    marche: RegionConfig,
}

pub(crate) async fn commands_handler(
    bot: Bot,
    msg: Message,
    cmd: Command,
    dynamodb_client: DynamoDbClient,
) -> Result<(), teloxide::RequestError> {
    let link_preview_options = link_preview_disabled();
    // Move it to /start only after a while to collect old users
    ensure_chat_presence(&dynamodb_client, &msg).await;
    let logger = logging::Logger::from_command(&cmd, &msg);

    let text = match cmd {
        Command::Help => Command::descriptions().to_string(),
        Command::Start => {
            let intro = if msg.chat.is_group() || msg.chat.is_supergroup() {
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
            };
            let text = format!(
                "{intro}\n\nSeleziona la regione da monitorare:\n\n{}",
                Command::descriptions()
            );
            let regions = match regions_config() {
                Ok(value) => value,
                Err(err) => {
                    logger.error(
                        "regions.config_missing",
                        &err,
                        "Missing regions configuration",
                    );
                    utils::send_message(
                        &bot,
                        &msg,
                        link_preview_options,
                        "Configurazione non disponibile. Riprova più tardi.",
                    )
                    .await?;
                    return Ok(());
                }
            };
            utils::send_message_with_markup(
                &bot,
                &msg,
                link_preview_options,
                &text,
                region_keyboard(regions),
            )
            .await?;
            return Ok(());
        }
        Command::CambiaRegione => {
            let text = "Scegli la regione da monitorare:";
            let regions = match regions_config() {
                Ok(value) => value,
                Err(err) => {
                    logger.error(
                        "regions.config_missing",
                        &err,
                        "Missing regions configuration",
                    );
                    utils::send_message(
                        &bot,
                        &msg,
                        link_preview_options,
                        "Configurazione non disponibile. Riprova più tardi.",
                    )
                    .await?;
                    return Ok(());
                }
            };
            utils::send_message_with_markup(
                &bot,
                &msg,
                link_preview_options,
                text,
                region_keyboard(regions),
            )
            .await?;
            return Ok(());
        }
        Command::Stazioni => {
            let Some(stations_table_name) =
                ensure_region_selected(&bot, &msg, &dynamodb_client, link_preview_options.clone())
                    .await?
            else {
                return Ok(());
            };
            let scan_page_size = stations_scan_page_size();
            match station::search::list_stations_cached(
                &dynamodb_client,
                stations_table_name.as_str(),
                scan_page_size,
            )
            .await
            {
                Ok(stations) if !stations.is_empty() => stations.join("\n"),
                Ok(_) => "Nessuna stazione disponibile al momento.".to_string(),
                Err(err) => {
                    logger.clone().table(stations_table_name).error(
                        "stations.list_failed",
                        &err,
                        "Failed to list stations",
                    );
                    "Errore nel recupero delle stazioni. Riprova più tardi.".to_string()
                }
            }
        }
        Command::Info => {
            let info = "Bot Telegram che permette di leggere i livelli idrometrici dei fiumi in Emilia-Romagna e Marche.\n\
                                I dati sono ottenuti da allertameteo.regione.emilia-romagna.it e dal portale app.protezionecivile.marche.it.\n\n\
                                Il progetto è completamente open-source (https://github.com/notdodo/erfiume_bot).\n\
                                Per sostenere e mantenere il servizio attivo: buymeacoffee.com/d0d0\n\n\
                                Inizia con /start o /stazioni, oppure cambia regione con /cambia_regione";
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
                        logger.clone().table(&alerts_table_name).error(
                            "alerts.list_failed",
                            &err,
                            "Failed to list alerts",
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
        Command::RimuoviAvviso(args) => {
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
                            logger.clone().table(&alerts_table_name).error(
                                "alerts.list_failed",
                                &err,
                                "Failed to list alerts",
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
                            logger
                                .clone()
                                .station(&alert.station_name)
                                .table(&alerts_table_name)
                                .error("alerts.delete_failed", &err, "Failed to delete alert");
                            false
                        }
                    };

                    if removed {
                        format!("Avviso rimosso per {}.", alert.station_name)
                    } else {
                        "Non ho trovato un avviso attivo per questa stazione.".to_string()
                    }
                } else {
                    let Some(stations_table_name) = ensure_region_selected(
                        &bot,
                        &msg,
                        &dynamodb_client,
                        link_preview_options.clone(),
                    )
                    .await?
                    else {
                        return Ok(());
                    };
                    let scan_page_size = stations_scan_page_size();
                    let station_result = station::search::get_station(
                        &dynamodb_client,
                        station_name,
                        stations_table_name.as_str(),
                        scan_page_size,
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
                            logger
                                .clone()
                                .station(&station.nomestaz)
                                .table(&alerts_table_name)
                                .error("alerts.delete_failed", &err, "Failed to delete alert");
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
                let Some(stations_table_name) = ensure_region_selected(
                    &bot,
                    &msg,
                    &dynamodb_client,
                    link_preview_options.clone(),
                )
                .await?
                else {
                    return Ok(());
                };
                let scan_page_size = stations_scan_page_size();
                let station_result = station::search::get_station(
                    &dynamodb_client,
                    station_name,
                    stations_table_name.as_str(),
                    scan_page_size,
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
                        logger.clone().station(&station.nomestaz).error(
                            "alerts.exists_check_failed",
                            &err,
                            "Failed to check alert existence",
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
                            logger.clone().table(&alerts_table_name).error(
                                "alerts.count_failed",
                                &err,
                                "Failed to count alerts",
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
                    logger
                        .clone()
                        .station(&station.nomestaz)
                        .table(&alerts_table_name)
                        .error("alerts.save_failed", &err, "Failed to save alert");
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

    let Some(stations_table_name) =
        ensure_region_selected(bot, msg, dynamodb_client, link_preview_options.clone()).await?
    else {
        return Ok(());
    };
    let scan_page_size = stations_scan_page_size();
    let station_query = text.trim().replace("@erfiume_bot", "").replace("/", "");
    let text = match station::search::get_station_with_match(
        dynamodb_client,
        station_query,
        stations_table_name.as_str(),
        scan_page_size,
    )
    .await
    {
        Ok(Some((item, match_kind))) => {
            let mut message = item.create_station_message().to_string();
            if matches!(match_kind, station::search::StationMatch::Fuzzy) {
                message.push_str(
                    "\nSe non è la stazione corretta prova ad affinare la ricerca.",
                );
            }
            message
        }
        Err(_) | Ok(None) => "Nessuna stazione trovata con la parola di ricerca.\nInserisci esattamente il nome che vedi nella pagina della regione selezionata:\n- Emilia-Romagna: https://allertameteo.regione.emilia-romagna.it/livello-idrometrico\n- Marche: http://app.protezionecivile.marche.it/sol/annaliidro2/index.sol?lang=it\nSe non sai quale cercare, prova con /stazioni oppure cambia regione con /cambia_regione.".to_string(),
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

pub(crate) async fn callback_query_handler(
    bot: Bot,
    query: CallbackQuery,
    dynamodb_client: DynamoDbClient,
) -> Result<(), teloxide::RequestError> {
    let Some(data) = query.data.as_deref() else {
        return Ok(());
    };

    let regions = match regions_config() {
        Ok(value) => value,
        Err(err) => {
            if let Some(message) = query.message.as_ref() {
                logger_from_callback_message(message).error(
                    "regions.config_missing",
                    &err,
                    "Missing regions configuration",
                );
            } else {
                logging::Logger::new().kind("callback_query").error(
                    "regions.config_missing",
                    &err,
                    "Missing regions configuration",
                );
            }
            bot.answer_callback_query(query.id)
                .text("Configurazione non disponibile.")
                .await?;
            return Ok(());
        }
    };

    let Some(region) = parse_region_callback_data(data, regions) else {
        bot.answer_callback_query(query.id)
            .text("Selezione non valida.")
            .await?;
        return Ok(());
    };

    let Some(message) = query.message.as_ref() else {
        bot.answer_callback_query(query.id)
            .text("Selezione non supportata.")
            .await?;
        return Ok(());
    };

    if let Some(regular_message) = message.regular_message() {
        ensure_chat_presence(&dynamodb_client, regular_message).await;
    }

    let chats_table_name = std::env::var("CHATS_TABLE_NAME").unwrap_or_default();
    if chats_table_name.is_empty() {
        bot.answer_callback_query(query.id)
            .text("Configurazione non disponibile.")
            .await?;
        return Ok(());
    }

    if let Err(err) = dynamo_chats::update_chat_region(
        &dynamodb_client,
        &chats_table_name,
        message.chat().id.0,
        region.key.as_str(),
    )
    .await
    {
        logger_from_callback_message(message)
            .table(&chats_table_name)
            .error(
                "chats.update_region_failed",
                &err,
                "Failed to save chat region",
            );
        bot.answer_callback_query(query.id)
            .text("Errore nel salvataggio. Riprova.")
            .await?;
        return Ok(());
    }

    logger_from_callback_message(message)
        .table(&chats_table_name)
        .info("chats.region_selected", "Region selected");

    bot.answer_callback_query(query.id)
        .text(format!("Regione impostata: {}.", region.label))
        .await?;

    let confirmation = format!(
        "Perfetto! Regione selezionata: {}.\n\nScrivi il nome di una stazione (e.g. `Cesena`) o usa /stazioni.",
        region.label
    );
    let edit_result = bot
        .edit_message_text(
            message.chat().id,
            message.id(),
            utils::escape_markdown_v2(&confirmation),
        )
        .parse_mode(ParseMode::MarkdownV2)
        .reply_markup(InlineKeyboardMarkup::new(
            Vec::<Vec<InlineKeyboardButton>>::new(),
        ))
        .await;
    if let Err(err) = edit_result {
        logger_from_callback_message(message).error(
            "message.edit_failed",
            &err,
            "Failed to edit region selection message",
        );
        if let Some(regular_message) = message.regular_message() {
            utils::send_message(
                &bot,
                regular_message,
                link_preview_disabled(),
                &confirmation,
            )
            .await?;
        } else {
            bot.send_message(message.chat().id, utils::escape_markdown_v2(&confirmation))
                .link_preview_options(link_preview_disabled())
                .parse_mode(ParseMode::MarkdownV2)
                .await?;
        }
    }

    Ok(())
}

fn logger_from_callback_message(message: &MaybeInaccessibleMessage) -> logging::Logger {
    message
        .regular_message()
        .map(logging::Logger::from_message)
        .unwrap_or_else(|| logging::Logger::new().kind("callback_query"))
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
        region: None,
        created_at: Utc::now().timestamp(),
    };

    if let Err(err) =
        dynamo_chats::insert_chat_if_missing(dynamodb_client, &chats_table_name, &record).await
    {
        logging::Logger::from_message(msg)
            .table(&chats_table_name)
            .error("chats.insert_failed", &err, "Failed to store chat");
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

fn region_keyboard(regions: &RegionsConfig) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            regions.emilia_romagna.label.as_str(),
            format!(
                "{REGION_CALLBACK_PREFIX}{}",
                regions.emilia_romagna.key.as_str()
            ),
        ),
        InlineKeyboardButton::callback(
            regions.marche.label.as_str(),
            format!("{REGION_CALLBACK_PREFIX}{}", regions.marche.key.as_str()),
        ),
    ]])
}

fn parse_region_callback_data<'a>(
    data: &str,
    regions: &'a RegionsConfig,
) -> Option<&'a RegionConfig> {
    let key = data.strip_prefix(REGION_CALLBACK_PREFIX)?;
    region_from_key(regions, key)
}

async fn ensure_region_selected(
    bot: &Bot,
    msg: &Message,
    dynamodb_client: &DynamoDbClient,
    link_preview_options: LinkPreviewOptions,
) -> Result<Option<String>, teloxide::RequestError> {
    let regions = match regions_config() {
        Ok(value) => value,
        Err(err) => {
            logging::Logger::from_message(msg).error(
                "regions.config_missing",
                &err,
                "Missing regions configuration",
            );
            utils::send_message(
                bot,
                msg,
                link_preview_options,
                "Configurazione non disponibile. Riprova più tardi.",
            )
            .await?;
            return Ok(None);
        }
    };

    let chats_table_name = std::env::var("CHATS_TABLE_NAME").unwrap_or_default();
    if chats_table_name.is_empty() {
        utils::send_message(
            bot,
            msg,
            link_preview_options,
            "Configurazione non disponibile. Riprova più tardi.",
        )
        .await?;
        return Ok(None);
    }

    match dynamo_chats::get_chat_region(dynamodb_client, &chats_table_name, msg.chat.id.0).await {
        Ok(Some(region_key)) => {
            if let Some(region) = region_from_key(regions, &region_key) {
                return Ok(Some(region.table_name.clone()));
            }
            logging::Logger::from_message(msg)
                .table(&chats_table_name)
                .info("chats.region_unknown", "Unknown region in chat record");
        }
        Ok(None) => {}
        Err(err) => {
            logging::Logger::from_message(msg)
                .table(&chats_table_name)
                .error(
                    "chats.region_lookup_failed",
                    &err,
                    "Failed to load chat region",
                );
            utils::send_message(
                bot,
                msg,
                link_preview_options,
                "Errore nel recupero della regione. Riprova più tardi.",
            )
            .await?;
            return Ok(None);
        }
    }

    let prompt = "Prima di continuare, scegli la regione da monitorare:";
    utils::send_message_with_markup(
        bot,
        msg,
        link_preview_options,
        prompt,
        region_keyboard(regions),
    )
    .await?;
    Ok(None)
}

fn region_from_key<'a>(regions: &'a RegionsConfig, key: &str) -> Option<&'a RegionConfig> {
    if key.eq_ignore_ascii_case(regions.emilia_romagna.key.as_str()) {
        Some(&regions.emilia_romagna)
    } else if key.eq_ignore_ascii_case(regions.marche.key.as_str()) {
        Some(&regions.marche)
    } else {
        None
    }
}

fn regions_config() -> Result<&'static RegionsConfig, String> {
    static CONFIG: OnceLock<Result<RegionsConfig, String>> = OnceLock::new();
    match CONFIG.get_or_init(load_regions_config) {
        Ok(config) => Ok(config),
        Err(err) => Err(err.clone()),
    }
}

fn load_regions_config() -> Result<RegionsConfig, String> {
    let emilia_romagna = RegionConfig {
        key: require_env("REGION_EMILIA_ROMAGNA_KEY")?,
        label: require_env("REGION_EMILIA_ROMAGNA_LABEL")?,
        table_name: require_env("EMILIA_ROMAGNA_STATIONS_TABLE_NAME")?,
    };
    let marche = RegionConfig {
        key: require_env("REGION_MARCHE_KEY")?,
        label: require_env("REGION_MARCHE_LABEL")?,
        table_name: require_env("MARCHE_STATIONS_TABLE_NAME")?,
    };
    Ok(RegionsConfig {
        emilia_romagna,
        marche,
    })
}

fn require_env(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Missing env var: {name}"))
}

fn stations_scan_page_size() -> i32 {
    let raw = std::env::var("STATIONS_SCAN_PAGE_SIZE").unwrap_or_default();
    let parsed = raw.trim().parse::<i32>().ok();
    let value = parsed.unwrap_or(DEFAULT_SCAN_PAGE_SIZE);
    value.clamp(1, MAX_SCAN_PAGE_SIZE)
}

#[cfg(test)]
mod tests {
    use erfiume_dynamodb::ALERT_ACTIVE;

    use crate::commands::utils::format_duration_millis;

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

    #[test]
    fn parse_region_callback_data_matches_known_regions() {
        let regions = sample_regions_config();
        assert!(
            parse_region_callback_data("region:emilia-romagna", &regions)
                .is_some_and(|region| region.key == "emilia-romagna")
        );
        assert!(
            parse_region_callback_data("region:marche", &regions)
                .is_some_and(|region| region.key == "marche")
        );
        assert!(parse_region_callback_data("region:unknown", &regions).is_none());
    }

    #[test]
    fn region_from_key_matches_and_unknown_returns_none() {
        let regions = sample_regions_config();
        assert!(
            region_from_key(&regions, "emilia-romagna")
                .is_some_and(|region| region.key == "emilia-romagna")
        );
        assert!(region_from_key(&regions, "Marche").is_some_and(|region| region.key == "marche"));
        assert!(region_from_key(&regions, "unknown").is_none());
    }

    fn sample_regions_config() -> RegionsConfig {
        RegionsConfig {
            emilia_romagna: RegionConfig {
                key: "emilia-romagna".to_string(),
                label: "Emilia-Romagna".to_string(),
                table_name: "EmiliaRomagna-Stations".to_string(),
            },
            marche: RegionConfig {
                key: "marche".to_string(),
                label: "Marche".to_string(),
                table_name: "Marche-Stations".to_string(),
            },
        }
    }
}
