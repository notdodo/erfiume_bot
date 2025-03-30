use teloxide::{
    payloads::SendMessageSetters,
    prelude::{Bot, Requester},
    types::{LinkPreviewOptions, Message, ParseMode},
};

fn escape_markdown_v2(text: &str) -> String {
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
