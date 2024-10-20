use dptree::deps;
use lambda_runtime::{service_fn, Error as LambdaError, LambdaEvent};
use serde_json::{json, Value};
use station::fuzzy::get_station;
use teloxide::{
    prelude::*,
    types::{LinkPreviewOptions, Me, ParseMode},
    utils::command::BotCommands,
};
use tracing::{info, instrument};
mod station;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client as DynamoDbClient;

use tracing_subscriber::EnvFilter;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum BaseCommand {
    /// Visualizza la lista dei comandi
    Help,
    /// Ottieni informazioni riguardanti il bot
    Info,
    ///  Inizia ad interagire con il bot
    Start,
    /// Visualizza la lista delle stazioni disponibili
    Stazioni,
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()) // Enable log level filtering via `RUST_LOG` env var
        .json()
        .with_current_span(false) // Optional: Exclude span information
        .with_span_list(false) // Optional: Exclude span list
        .with_target(false)
        .without_time()
        .init();

    let func = service_fn(lambda_handler);
    lambda_runtime::run(func).await?;
    Ok(())
}

#[instrument]
async fn lambda_handler(event: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    let bot = Bot::from_env();
    let me: Me = bot.get_me().await?;
    info!("{:?}", event.payload);

    let outer_json: Value = serde_json::from_value(
        event
            .payload
            .get("body")
            .ok_or_else(|| LambdaError::from("Missing 'body' in event payload"))?
            .clone(),
    )?;
    let inner_json_str = outer_json
        .as_str()
        .ok_or_else(|| LambdaError::from("Expected 'body' to be a string"))?;
    let update: Update = serde_json::from_str(inner_json_str)?;

    let handler = Update::filter_message()
        .branch(
            dptree::entry()
                .filter_command::<BaseCommand>()
                .endpoint(simple_commands_handler),
        )
        .branch(dptree::endpoint(|msg: Message, bot: Bot| async move {
            let shared_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
            let dynamodb_client = DynamoDbClient::new(&shared_config);
            let message = msg.text().unwrap();
            let stations = station::stations();
            let closest_station = stations.iter().min_by_key(|&s| {
                edit_distance::edit_distance(
                    &message.to_lowercase(),
                    &s.replace(" ", "").to_lowercase(),
                )
            });
            let text = match get_station(
                &dynamodb_client,
                closest_station.unwrap().to_string(),
                "Stazioni",
            )
            .await
            {
                Ok(item) => {
                    if item.nomestaz != message {
                        format!("{}\nSe non è la stazione corretta prova ad affinare la ricerca.", item.create_station_message())
                    }else {
                        item.create_station_message().to_string()
                    }
                }
                Err(_) => "Nessuna stazione trovata con la parola di ricerca. \n
                            Inserisci esattamente il nome che vedi dalla pagina https://allertameteo.regione.emilia-romagna.it/livello-idrometrico \n
                            Ad esempio 'Cesena', 'Lavino di Sopra' o 'S. Carlo'. \n
                            Se non sai quale cercare prova con /stazioni".to_string(),
            };
            bot.send_message(msg.chat.id, text).await?;
            respond(())
        }));

    handler.dispatch(deps![me, bot, update]).await;
    Ok(json!({
        "message": "Lambda executed successfully",
        "statusCode": 200,
    }))
}

fn escape_markdown_v2(text: &str) -> String {
    text.replace("\\", "\\\\")
        .replace("_", "\\_")
        .replace("*", "\\*")
        .replace("[", "\\[")
        .replace("]", "\\]")
        .replace("(", "\\(")
        .replace(")", "\\)")
        .replace("~", "\\~")
        .replace("`", "\\`")
        .replace(">", "\\>")
        .replace("#", "\\#")
        .replace("+", "\\+")
        .replace("-", "\\-")
        .replace("=", "\\=")
        .replace("|", "\\|")
        .replace("{", "\\{")
        .replace("}", "\\}")
        .replace(".", "\\.")
        .replace("!", "\\!")
}

async fn simple_commands_handler(
    bot: Bot,
    msg: Message,
    cmd: BaseCommand,
) -> Result<(), teloxide::RequestError> {
    let text = match cmd {
        BaseCommand::Help => BaseCommand::descriptions().to_string(),
        BaseCommand::Start => {
            if msg.chat.is_group() || msg.chat.is_supergroup() {
                format!("Ciao {}! Scrivete il nome di una stazione da monitorare (e.g. /Cesena o `/S. Carlo`) 
                        o cercatene una con /stazioni",
                        msg.chat.title().unwrap_or(""))
            } else {
                format!("Ciao @{}! Scrivi il nome di una stazione da monitorare (e.g. `Cesena` o `/S. Carlo`) \
                        o cercane una con /stazioni",
                        msg.chat.username().unwrap_or(msg.chat.first_name().unwrap_or("")))
            }
        }
        BaseCommand::Stazioni => station::stations().join("\n"),
        BaseCommand::Info => {
            let info = "Bot Telegram che permette di leggere i livello idrometrici dei fiumi dell'Emilia Romagna \
                              I dati idrometrici sono ottenuti dalle API messe a disposizione da allertameteo.regione.emilia-romagn.i.\n\n\
                              Il progetto è completamente open-source (https://github.com/notdodo/erfiume_bot).\n\
                              Per donazioni per mantenere il servizio attivo: buymeacoffee.com/d0d0\n\n\
                              Inizia con /start o /stazioni";
            info.to_string()
        }
    };

    bot.send_message(msg.chat.id, escape_markdown_v2(&text))
        .link_preview_options(LinkPreviewOptions {
            is_disabled: true,
            url: None,
            prefer_small_media: false,
            prefer_large_media: false,
            show_above_text: false,
        })
        .parse_mode(ParseMode::MarkdownV2)
        .await?;

    Ok(())
}
