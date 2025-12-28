use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use lambda_runtime::{Error as LambdaError, LambdaEvent, service_fn};
use serde_json::{Value, json};
use teloxide::{
    dispatching::{HandlerExt, UpdateFilterExt},
    dptree::deps,
    payloads::{DeleteMyCommandsSetters, GetMyCommandsSetters, SetMyCommandsSetters},
    prelude::{Bot, Requester, Update, dptree},
    respond,
    types::BotCommandScope,
    types::Me,
    utils::command::BotCommands,
};
use tracing::instrument;
use tracing_subscriber::EnvFilter;
mod commands;
mod logging;
mod station;

struct AppState {
    dynamodb_client: DynamoDbClient,
    bot: Bot,
    me: Me,
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

    let bot = Bot::from_env();
    configure_bot_commands(&bot).await;
    let me = bot.get_me().await?;
    let dynamodb_client =
        DynamoDbClient::new(&aws_config::defaults(BehaviorVersion::latest()).load().await);

    let app_state = AppState {
        dynamodb_client,
        bot,
        me,
    };

    lambda_runtime::run(service_fn(|event: LambdaEvent<Value>| async {
        lambda_handler(&app_state, event).await
    }))
    .await?;
    Ok(())
}

async fn configure_bot_commands(bot: &Bot) {
    let commands = commands::Command::bot_commands();

    set_commands(bot, &commands, BotCommandScope::Default, None).await;
    set_commands(bot, &commands, BotCommandScope::AllPrivateChats, None).await;
    set_commands(bot, &commands, BotCommandScope::AllGroupChats, None).await;
    set_commands(bot, &commands, BotCommandScope::AllChatAdministrators, None).await;

    delete_language_commands(bot, BotCommandScope::Default, "en").await;
    delete_language_commands(bot, BotCommandScope::Default, "it").await;
    delete_language_commands(bot, BotCommandScope::AllPrivateChats, "en").await;
    delete_language_commands(bot, BotCommandScope::AllPrivateChats, "it").await;
    delete_language_commands(bot, BotCommandScope::AllGroupChats, "en").await;
    delete_language_commands(bot, BotCommandScope::AllGroupChats, "it").await;
    delete_language_commands(bot, BotCommandScope::AllChatAdministrators, "en").await;
    delete_language_commands(bot, BotCommandScope::AllChatAdministrators, "it").await;

    log_commands(bot, BotCommandScope::Default, None).await;
    log_commands(bot, BotCommandScope::AllPrivateChats, None).await;
    log_commands(bot, BotCommandScope::AllGroupChats, None).await;
    log_commands(bot, BotCommandScope::AllChatAdministrators, None).await;
}

async fn set_commands(
    bot: &Bot,
    commands: &[teloxide::types::BotCommand],
    scope: BotCommandScope,
    language_code: Option<&'static str>,
) {
    let mut request = bot.set_my_commands(commands.to_vec());
    request = SetMyCommandsSetters::scope(request, scope);
    if let Some(language_code) = language_code {
        request = SetMyCommandsSetters::language_code(request, language_code);
    }
    if let Err(err) = request.await {
        let logger = logging::Logger::new();
        logger.error(
            "bot.commands.set_failed",
            &err,
            "Failed to set bot commands",
        );
    }
}

async fn delete_language_commands(bot: &Bot, scope: BotCommandScope, language_code: &'static str) {
    let mut request = bot.delete_my_commands().scope(scope);
    request = request.language_code(language_code);
    if let Err(err) = request.await {
        let logger = logging::Logger::new().language_code(language_code);
        logger.error(
            "bot.commands.delete_failed",
            &err,
            "Failed to delete language-specific commands",
        );
    }
}

async fn log_commands(bot: &Bot, scope: BotCommandScope, language_code: Option<&'static str>) {
    let mut request = bot.get_my_commands().scope(scope);
    if let Some(language_code) = language_code {
        request = request.language_code(language_code);
    }
    match request.await {
        Ok(commands) => {
            let names: Vec<String> = commands
                .iter()
                .map(|command| command.command.clone())
                .collect();
            let logger = logging::Logger::new()
                .language_code(language_code.unwrap_or("default"))
                .command_count(commands.len())
                .commands(names.join(","));
            logger.info("bot.commands.list", "Bot command list");
        }
        Err(err) => {
            let logger = logging::Logger::new().language_code(language_code.unwrap_or("default"));
            logger.error(
                "bot.commands.read_failed",
                &err,
                "Failed to read bot commands",
            );
        }
    }
}

#[instrument(skip(app_state, event))]
async fn lambda_handler(
    app_state: &AppState,
    event: LambdaEvent<Value>,
) -> Result<Value, LambdaError> {
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
    logging::update_summary(&update);

    let handler = Update::filter_message()
        .branch(
            dptree::entry()
                .filter_command::<commands::Command>()
                .endpoint(commands::commands_handler),
        )
        .branch(dptree::endpoint(|msg, bot, dynamodb_client| async move {
            commands::message_handler(&bot, &msg, &dynamodb_client).await?;
            respond(())
        }));

    let _ = handler
        .dispatch(deps![
            app_state.me.clone(),
            app_state.bot.clone(),
            update,
            app_state.dynamodb_client.clone()
        ])
        .await;
    Ok(json!({
        "message": "Lambda executed successfully",
        "statusCode": 200,
    }))
}
