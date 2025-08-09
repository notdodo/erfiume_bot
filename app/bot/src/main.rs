use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use lambda_runtime::{Error as LambdaError, LambdaEvent, service_fn};
use serde_json::{Value, json};
use teloxide::{
    dispatching::{HandlerExt, UpdateFilterExt},
    dptree::deps,
    prelude::{Bot, Requester, Update, dptree},
    respond,
    types::Me,
};
use tracing::{info, instrument};
use tracing_subscriber::EnvFilter;
mod commands;
mod station;

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

    lambda_runtime::run(service_fn(|event: LambdaEvent<Value>| async {
        lambda_handler(
            &DynamoDbClient::new(&aws_config::defaults(BehaviorVersion::latest()).load().await),
            event,
        )
        .await
    }))
    .await?;
    Ok(())
}

#[instrument]
async fn lambda_handler(
    dynamodb_client: &DynamoDbClient,
    event: LambdaEvent<Value>,
) -> Result<Value, LambdaError> {
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
                .filter_command::<commands::BaseCommand>()
                .endpoint(commands::base_commands_handler),
        )
        .branch(dptree::endpoint(|msg, bot, dynamodb_client| async move {
            commands::message_handler(&bot, &msg, &dynamodb_client).await?;
            respond(())
        }));

    let _ = handler
        .dispatch(deps![me, bot, update, dynamodb_client.clone()])
        .await;
    Ok(json!({
        "message": "Lambda executed successfully",
        "statusCode": 200,
    }))
}
