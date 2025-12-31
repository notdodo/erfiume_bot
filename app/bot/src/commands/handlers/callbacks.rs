use super::regions::{parse_region_callback_data, regions_config};
use crate::commands::utils;
use crate::logging;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use chrono::Utc;
use erfiume_dynamodb::chats as dynamo_chats;
use teloxide::payloads::{AnswerCallbackQuerySetters, EditMessageTextSetters, SendMessageSetters};
use teloxide::prelude::{Bot, Requester};
use teloxide::types::{
    CallbackQuery, Chat, InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage,
    ParseMode,
};

pub(crate) async fn callback_query_handler(
    bot: Bot,
    query: CallbackQuery,
    dynamodb_client: DynamoDbClient,
) -> Result<(), teloxide::RequestError> {
    let Some(data) = query.data.as_deref() else {
        return Ok(());
    };

    let callback_logger = query
        .message
        .as_ref()
        .map(logger_from_callback_message)
        .unwrap_or_else(|| logging::Logger::new().kind("callback_query"));

    let regions = match regions_config() {
        Ok(value) => value,
        Err(err) => {
            callback_logger.error(
                "regions.config_missing",
                &err,
                "Missing regions configuration",
            );
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

    let chats_table_name = match std::env::var("CHATS_TABLE_NAME") {
        Ok(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => {
            bot.answer_callback_query(query.id)
                .text("Configurazione non disponibile.")
                .await?;
            return Ok(());
        }
    };

    let chat = message.chat();
    let record = dynamo_chats::ChatRecord {
        chat_id: chat.id.0,
        chat_type: chat_type_name(chat).to_string(),
        username: chat.username().map(|value| value.to_string()),
        first_name: chat.first_name().map(|value| value.to_string()),
        last_name: chat.last_name().map(|value| value.to_string()),
        title: chat.title().map(|value| value.to_string()),
        region: None,
        created_at: Utc::now().timestamp(),
    };

    if let Err(err) = dynamo_chats::upsert_chat_region(
        &dynamodb_client,
        &chats_table_name,
        &record,
        region.key.as_str(),
    )
    .await
    {
        callback_logger.clone().table(&chats_table_name).error(
            "chats.update_region_failed",
            &err,
            "Failed to save chat region",
        );
        bot.answer_callback_query(query.id)
            .text("Errore nel salvataggio. Riprova.")
            .await?;
        return Ok(());
    }

    callback_logger
        .clone()
        .table(&chats_table_name)
        .info("chats.region_selected", "Region selected");

    bot.answer_callback_query(query.id)
        .text(format!("Regione impostata: {}.", region.label))
        .await?;

    let confirmation = format!(
        "Perfetto! Regione selezionata: {}.\n\nScrivi il nome di una stazione (e.g. `Cesena` o /Pianello) o usa /stazioni.",
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
        callback_logger.error(
            "message.edit_failed",
            &err,
            "Failed to edit region selection message",
        );
        if let Some(regular_message) = message.regular_message() {
            utils::send_message(
                &bot,
                regular_message,
                utils::link_preview_disabled(),
                &confirmation,
            )
            .await?;
        } else {
            bot.send_message(message.chat().id, utils::escape_markdown_v2(&confirmation))
                .link_preview_options(utils::link_preview_disabled())
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

fn chat_type_name(chat: &Chat) -> &'static str {
    if chat.is_private() {
        "private"
    } else if chat.is_group() {
        "group"
    } else if chat.is_supergroup() {
        "supergroup"
    } else if chat.is_channel() {
        "channel"
    } else {
        "other"
    }
}
