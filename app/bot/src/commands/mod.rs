use aws_sdk_dynamodb::Client as DynamoDbClient;
use teloxide::{
    prelude::Bot,
    types::{LinkPreviewOptions, Message},
    utils::command::BotCommands,
};

use crate::station;
pub(crate) mod utils;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub(crate) enum BaseCommand {
    /// Visualizza la lista dei comandi
    Help,
    /// Ottieni informazioni riguardanti il bot
    Info,
    /// Inizia ad interagire con il bot
    Start,
    /// Visualizza la lista delle stazioni disponibili
    Stazioni,
}

pub(crate) async fn base_commands_handler(
    bot: Bot,
    msg: Message,
    cmd: BaseCommand,
) -> Result<(), teloxide::RequestError> {
    let text = match cmd {
        BaseCommand::Help => BaseCommand::descriptions().to_string(),
        BaseCommand::Start => {
            if msg.chat.is_group() || msg.chat.is_supergroup() {
                format!("Ciao {}! Scrivete il nome di una stazione da monitorare (e.g. /Cesena@erfiume_bot o /Borello@erfiume_bot) \
                        o cercatene una con /stazioni@erfiume_bot",
                        msg.chat.title().unwrap_or(""))
            } else {
                format!("Ciao @{}! Scrivi il nome di una stazione da monitorare (e.g. `Cesena` o /SCarlo) \
                        o cercane una con /stazioni",
                        msg.chat.username().unwrap_or(msg.chat.first_name().unwrap_or("")))
            }
        }
        BaseCommand::Stazioni => station::stations().join("\n"),
        BaseCommand::Info => {
            let info = "Bot Telegram che permette di leggere i livello idrometrici dei fiumi dell'Emilia Romagna \
                              I dati idrometrici sono ottenuti dalle API messe a disposizione da allertameteo.regione.emilia-romagna.it\n\n\
                              Il progetto è completamente open-source (https://github.com/notdodo/erfiume_bot).\n\
                              Per donazioni per mantenere il servizio attivo: buymeacoffee.com/d0d0\n\n\
                              Inizia con /start o /stazioni";
            info.to_string()
        }
    };

    utils::send_message(
        &bot,
        &msg,
        LinkPreviewOptions {
            is_disabled: true,
            url: None,
            prefer_small_media: false,
            prefer_large_media: false,
            show_above_text: false,
        },
        &text,
    )
    .await?;

    Ok(())
}

pub(crate) async fn message_handler(
    bot: &Bot,
    msg: &Message,
    dynamodb_client: DynamoDbClient,
) -> Result<(), teloxide::RequestError> {
    let link_preview_options = LinkPreviewOptions {
        is_disabled: false,
        url: None,
        prefer_small_media: true,
        prefer_large_media: false,
        show_above_text: false,
    };

    let Some(text) = msg.text() else {
        return Ok(()); // Do nothing if the message has no text
    };

    let text = match station::search::get_station(
        &dynamodb_client,
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
        Err(_) | Ok(None) => "Nessuna stazione trovata con la parola di ricerca.\nInserisci esattamente il nome che vedi dalla pagina https://allertameteo.regione.emilia-romagna.it/livello-idrometrico\nAd esempio 'Cesena', 'Lavino di Sopra' o 'S. Carlo'.\nSe non sai quale cercare prova con /stazioni".to_string(),
    };

    let mut message = text.clone();
    if fastrand::choose_multiple(0..10, 1)[0] == 8 {
        message = format!(
            "{}\n\nContribuisci al progetto per mantenerlo attivo e sviluppare nuove funzionalità tramite una donazione: https://buymeacoffee.com/d0d0",
            text
        );
    }
    if fastrand::choose_multiple(0..50, 1)[0] == 8 {
        message = format!(
            "{}\n\nEsplora o contribuisci al progetto open-source per sviluppare nuove funzionalità: https://github.com/notdodo/erfiume_bot",
            text
        );
    }
    utils::send_message(bot, msg, link_preview_options, &message).await?;

    Ok(())
}
