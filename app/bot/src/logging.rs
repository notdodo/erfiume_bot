use teloxide::types::{Message, Update, UpdateKind};
use tracing::{error, info};

use crate::commands::Command;
use crate::commands::utils::chat_type_name;

pub(crate) const TARGET: &str = "erfiume_bot";

#[derive(Clone, Default)]
pub(crate) struct Logger {
    command: Option<&'static str>,
    chat_id: Option<i64>,
    chat_type: Option<&'static str>,
    message_id: Option<i32>,
    station: Option<String>,
    table: Option<String>,
    language_code: Option<&'static str>,
    update_id: Option<u32>,
    kind: Option<&'static str>,
    text: Option<String>,
}

impl Logger {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn from_message(msg: &Message) -> Self {
        Self {
            chat_id: Some(msg.chat.id.0),
            chat_type: Some(chat_type_name(&msg.chat)),
            message_id: Some(msg.id.0),
            ..Self::default()
        }
    }

    pub(crate) fn from_command(cmd: &Command, msg: &Message) -> Self {
        Self {
            command: Some(command_name(cmd)),
            chat_id: Some(msg.chat.id.0),
            chat_type: Some(chat_type_name(&msg.chat)),
            message_id: Some(msg.id.0),
            ..Self::default()
        }
    }

    pub(crate) fn station(mut self, station: impl Into<String>) -> Self {
        self.station = Some(station.into());
        self
    }

    pub(crate) fn table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(table.into());
        self
    }

    pub(crate) fn update_id(mut self, update_id: u32) -> Self {
        self.update_id = Some(update_id);
        self
    }

    pub(crate) fn kind(mut self, kind: &'static str) -> Self {
        self.kind = Some(kind);
        self
    }

    pub(crate) fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub(crate) fn info(&self, event: &'static str, message: &str) {
        let station = self.station.as_deref();
        let table = self.table.as_deref();
        info!(
            target: TARGET,
            event,
            command = self.command,
            chat_id = self.chat_id,
            chat_type = self.chat_type,
            message_id = self.message_id,
            station = station,
            table = table,
            language_code = self.language_code,
            update_id = self.update_id,
            kind = self.kind,
            text = self.text.as_deref(),
            "{}",
            message
        );
    }

    pub(crate) fn error<E: std::fmt::Debug>(&self, event: &'static str, err: &E, message: &str) {
        let station = self.station.as_deref();
        let table = self.table.as_deref();
        error!(
            target: TARGET,
            event,
            command = self.command,
            chat_id = self.chat_id,
            chat_type = self.chat_type,
            message_id = self.message_id,
            station = station,
            table = table,
            language_code = self.language_code,
            update_id = self.update_id,
            kind = self.kind,
            text = self.text.as_deref(),
            error = ?err,
            "{}",
            message
        );
    }
}

pub(crate) fn update_summary(update: &Update) {
    let kind = update_kind_name(update);
    if let UpdateKind::Message(msg) = &update.kind {
        let mut text_preview = msg.text().unwrap_or("").to_string();
        const MAX_LEN: usize = 400;
        if text_preview.chars().count() > MAX_LEN {
            text_preview = text_preview.chars().take(MAX_LEN).collect();
            text_preview.push_str("...");
        }
        let logger = Logger::from_message(msg)
            .update_id(update.id.0)
            .kind(kind)
            .text(text_preview);
        logger.info("update.received", "update");
    } else {
        let logger = Logger::new().update_id(update.id.0).kind(kind);
        logger.info("update.received", "update");
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

fn command_name(cmd: &Command) -> &'static str {
    match cmd {
        Command::Help => "help",
        Command::Info => "info",
        Command::Start => "start",
        Command::Stazioni => "stazioni",
        Command::Avvisami(_) => "avvisami",
        Command::ListaAvvisi => "lista_avvisi",
        Command::CambiaRegione => "cambia_regione",
        Command::RimuoviAvviso(_) => "rimuovi_avviso",
    }
}
