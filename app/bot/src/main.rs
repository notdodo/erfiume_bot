use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use lambda_runtime::{Error as LambdaError, LambdaEvent, service_fn};
use serde_json::{Value, json};
use teloxide::{
    dispatching::{HandlerExt, UpdateFilterExt},
    dptree::deps,
    prelude::{Bot, Requester, Update, dptree},
    respond,
    types::{Me, UpdateKind},
};
use tracing::{info, instrument};
use tracing_subscriber::EnvFilter;
mod commands;
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
    log_update_summary(&update);

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

fn log_update_summary(update: &Update) {
    let kind = update_kind_name(update);
    if let UpdateKind::Message(msg) = &update.kind {
        let mut text_preview = msg.text().unwrap_or("").to_string();
        const MAX_LEN: usize = 400;
        if text_preview.chars().count() > MAX_LEN {
            text_preview = text_preview.chars().take(MAX_LEN).collect();
            text_preview.push_str("...");
        }
        info!(
            target: "erfiume_bot",
            update_id = update.id.0,
            kind,
            chat_id = msg.chat.id.0,
            message_id = msg.id.0,
            text = %text_preview,
            "update"
        );
    } else {
        info!(
            target: "erfiume_bot",
            update_id = update.id.0,
            kind,
            "update"
        );
    }
}

fn update_kind_name(update: &Update) -> &'static str {
    match &update.kind {
        UpdateKind::Message(_) => "message",
        UpdateKind::EditedMessage(_) => "edited_message",
        UpdateKind::ChannelPost(_) => "channel_post",
        UpdateKind::EditedChannelPost(_) => "edited_channel_post",
        UpdateKind::InlineQuery(_) => "inline_query",
        UpdateKind::ChosenInlineResult(_) => "chosen_inline_result",
        UpdateKind::CallbackQuery(_) => "callback_query",
        UpdateKind::ShippingQuery(_) => "shipping_query",
        UpdateKind::PreCheckoutQuery(_) => "pre_checkout_query",
        UpdateKind::Poll(_) => "poll",
        UpdateKind::PollAnswer(_) => "poll_answer",
        UpdateKind::MyChatMember(_) => "my_chat_member",
        UpdateKind::ChatMember(_) => "chat_member",
        UpdateKind::ChatJoinRequest(_) => "chat_join_request",
        _ => "other",
    }
}
