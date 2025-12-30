use erfiume_dynamodb::ALERT_ACTIVE;
use erfiume_dynamodb::alerts as dynamo_alerts;
use teloxide::{
    payloads::SendMessageSetters,
    prelude::{Bot, Requester},
    types::{LinkPreviewOptions, Message, ParseMode, ReplyMarkup},
};

pub(crate) fn escape_markdown_v2(text: &str) -> String {
    text.replace("\\", "\\\\")
        .replace("_", "\\_")
        .replace("*", "\\*")
        .replace("[", "\\[")
        .replace("]", "\\]")
        .replace("(", "\\(")
        .replace(")", "\\)")
        .replace("~", "\\~")
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

pub(crate) async fn send_message(
    bot: &Bot,
    msg: &Message,
    preview_options: LinkPreviewOptions,
    text: &str,
) -> Result<teloxide::prelude::Message, teloxide::RequestError> {
    if let Some(thread_id) = msg.thread_id {
        bot.send_message(msg.chat.id, escape_markdown_v2(text))
            .link_preview_options(preview_options)
            .message_thread_id(thread_id)
            .parse_mode(ParseMode::MarkdownV2)
            .await
    } else {
        bot.send_message(msg.chat.id, escape_markdown_v2(text))
            .link_preview_options(preview_options)
            .parse_mode(ParseMode::MarkdownV2)
            .await
    }
}

pub(crate) async fn send_message_with_markup(
    bot: &Bot,
    msg: &Message,
    preview_options: LinkPreviewOptions,
    text: &str,
    reply_markup: impl Into<ReplyMarkup>,
) -> Result<teloxide::prelude::Message, teloxide::RequestError> {
    if let Some(thread_id) = msg.thread_id {
        bot.send_message(msg.chat.id, escape_markdown_v2(text))
            .link_preview_options(preview_options)
            .message_thread_id(thread_id)
            .parse_mode(ParseMode::MarkdownV2)
            .reply_markup(reply_markup)
            .await
    } else {
        bot.send_message(msg.chat.id, escape_markdown_v2(text))
            .link_preview_options(preview_options)
            .parse_mode(ParseMode::MarkdownV2)
            .reply_markup(reply_markup)
            .await
    }
}

pub(crate) fn format_alert_status(alert: &dynamo_alerts::AlertEntry, now_millis: u64) -> String {
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

pub(crate) fn format_duration_millis(millis: u64) -> String {
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

pub(crate) fn link_preview_disabled() -> LinkPreviewOptions {
    LinkPreviewOptions {
        is_disabled: true,
        url: None,
        prefer_small_media: false,
        prefer_large_media: false,
        show_above_text: false,
    }
}

pub(crate) fn link_preview_small_media() -> LinkPreviewOptions {
    LinkPreviewOptions {
        is_disabled: false,
        url: None,
        prefer_small_media: true,
        prefer_large_media: false,
        show_above_text: false,
    }
}
