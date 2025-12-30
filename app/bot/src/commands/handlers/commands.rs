use super::alerts;
use super::regions::{
    ensure_region_selected, load_region_for_chat, region_keyboard, regions_config,
    stations_scan_page_size,
};
use crate::commands::context::ChatContext;
use crate::commands::utils;
use crate::{logging, station};
use aws_sdk_dynamodb::Client as DynamoDbClient;
use teloxide::prelude::Bot;
use teloxide::types::{LinkPreviewOptions, Message, ReplyMarkup};
use teloxide::utils::command::BotCommands;

use super::super::Command;

pub(crate) async fn commands_handler(
    bot: Bot,
    msg: Message,
    cmd: Command,
    dynamodb_client: DynamoDbClient,
) -> Result<(), teloxide::RequestError> {
    let ctx = ChatContext::from_message(&dynamodb_client, &msg);
    let logger = logging::Logger::from_command(&cmd, &msg);
    let handler = CommandHandler::new(&bot, &msg, ctx, logger);
    handler.handle(cmd).await
}

pub(super) struct CommandHandler<'a> {
    bot: &'a Bot,
    msg: &'a Message,
    ctx: ChatContext,
    logger: logging::Logger,
    link_preview_options: LinkPreviewOptions,
}

impl<'a> CommandHandler<'a> {
    fn new(bot: &'a Bot, msg: &'a Message, ctx: ChatContext, logger: logging::Logger) -> Self {
        Self {
            bot,
            msg,
            ctx,
            logger,
            link_preview_options: utils::link_preview_disabled(),
        }
    }

    async fn handle(&self, cmd: Command) -> Result<(), teloxide::RequestError> {
        match cmd {
            Command::Help => self.handle_help().await,
            Command::Start => self.handle_start().await,
            Command::CambiaRegione => self.handle_cambia_regione().await,
            Command::Stazioni => self.handle_stazioni().await,
            Command::Info => self.handle_info().await,
            Command::ListaAvvisi => alerts::handle_lista_avvisi(self).await,
            Command::RimuoviAvviso(args) => alerts::handle_rimuovi_avviso(self, args).await,
            Command::Avvisami(args) => alerts::handle_avvisami(self, args).await,
        }
    }

    pub(super) fn bot(&self) -> &Bot {
        self.bot
    }

    pub(super) fn msg(&self) -> &Message {
        self.msg
    }

    pub(super) fn ctx(&self) -> &ChatContext {
        &self.ctx
    }

    pub(super) fn logger(&self) -> &logging::Logger {
        &self.logger
    }

    pub(super) fn link_preview_options(&self) -> LinkPreviewOptions {
        self.link_preview_options.clone()
    }

    pub(super) fn dynamodb(&self) -> &DynamoDbClient {
        self.ctx.dynamodb_client()
    }

    pub(super) async fn send_text(&self, text: &str) -> Result<(), teloxide::RequestError> {
        utils::send_message(self.bot, self.msg, self.link_preview_options.clone(), text).await?;
        Ok(())
    }

    pub(super) async fn send_text_with_markup(
        &self,
        text: &str,
        reply_markup: impl Into<ReplyMarkup>,
    ) -> Result<(), teloxide::RequestError> {
        utils::send_message_with_markup(
            self.bot,
            self.msg,
            self.link_preview_options.clone(),
            text,
            reply_markup,
        )
        .await?;
        Ok(())
    }

    async fn handle_help(&self) -> Result<(), teloxide::RequestError> {
        self.send_text(&Command::descriptions().to_string()).await
    }

    async fn handle_start(&self) -> Result<(), teloxide::RequestError> {
        let intro = if self.msg.chat.is_group() || self.msg.chat.is_supergroup() {
            format!(
                "Ciao {}! Scrivete il nome di una stazione da monitorare (e.g. /Cesena@erfiume_bot o /Pianello@erfiume_bot) \
                        o cercatene una con /stazioni@erfiume_bot",
                self.msg.chat.title().unwrap_or("")
            )
        } else {
            format!(
                "Ciao @{}! Scrivi il nome di una stazione da monitorare (e.g. `Cesena`, /Marotta o /SCarlo) \
                        o cercane una con /stazioni",
                self.msg
                    .chat
                    .username()
                    .unwrap_or(self.msg.chat.first_name().unwrap_or(""))
            )
        };

        let regions = match regions_config() {
            Ok(value) => value,
            Err(err) => {
                self.logger.error(
                    "regions.config_missing",
                    &err,
                    "Missing regions configuration",
                );
                self.send_text("Configurazione non disponibile. Riprova più tardi.")
                    .await?;
                return Ok(());
            }
        };

        let region = match load_region_for_chat(&self.ctx, &self.logger, regions).await {
            Ok(region) => region,
            Err(message) => {
                self.send_text(message).await?;
                return Ok(());
            }
        };

        let has_region = region.is_some();
        let text = if let Some(region) = region {
            format!(
                "{intro}\nRegione attuale: {}.\n\n{}",
                region.label,
                Command::descriptions()
            )
        } else {
            format!(
                "{intro}\n\nSeleziona la regione da monitorare:\n\n{}",
                Command::descriptions()
            )
        };

        if has_region {
            self.send_text(&text).await
        } else {
            self.send_text_with_markup(&text, region_keyboard(regions))
                .await
        }
    }

    async fn handle_cambia_regione(&self) -> Result<(), teloxide::RequestError> {
        let regions = match regions_config() {
            Ok(value) => value,
            Err(err) => {
                self.logger.error(
                    "regions.config_missing",
                    &err,
                    "Missing regions configuration",
                );
                self.send_text("Configurazione non disponibile. Riprova più tardi.")
                    .await?;
                return Ok(());
            }
        };

        self.send_text_with_markup("Scegli la regione da monitorare:", region_keyboard(regions))
            .await
    }

    async fn handle_stazioni(&self) -> Result<(), teloxide::RequestError> {
        let Some(stations_table_name) = ensure_region_selected(
            &self.ctx,
            self.bot,
            self.msg,
            self.link_preview_options.clone(),
        )
        .await?
        else {
            return Ok(());
        };
        let scan_page_size = stations_scan_page_size();
        let text = match station::search::list_stations_cached(
            self.dynamodb(),
            stations_table_name.as_str(),
            scan_page_size,
        )
        .await
        {
            Ok(stations) if !stations.is_empty() => stations.join("\n"),
            Ok(_) => "Nessuna stazione disponibile al momento.".to_string(),
            Err(err) => {
                self.logger.clone().table(stations_table_name).error(
                    "stations.list_failed",
                    &err,
                    "Failed to list stations",
                );
                "Errore nel recupero delle stazioni. Riprova più tardi.".to_string()
            }
        };
        self.send_text(&text).await
    }

    async fn handle_info(&self) -> Result<(), teloxide::RequestError> {
        let region_line = match regions_config() {
            Ok(regions) => match load_region_for_chat(&self.ctx, &self.logger, regions).await {
                Ok(Some(region)) => format!("Regione attuale: {}.", region.label),
                Ok(None) => "Regione attuale: non impostata.".to_string(),
                Err(_) => "Regione attuale: non disponibile.".to_string(),
            },
            Err(err) => {
                self.logger.error(
                    "regions.config_missing",
                    &err,
                    "Missing regions configuration",
                );
                "Regione attuale: non disponibile.".to_string()
            }
        };

        let text = format!(
            "Bot Telegram che permette di leggere i livelli idrometrici dei fiumi in Emilia-Romagna e Marche.\n\
                I dati sono ottenuti da allertameteo.regione.emilia-romagna.it e dal portale app.protezionecivile.marche.it.\n\n\
                {region_line}\n\n\
                Il progetto è completamente open-source (https://github.com/notdodo/erfiume_bot).\n\
                Per sostenere e mantenere il servizio attivo: buymeacoffee.com/d0d0\n\n\
                Inizia con /start o /stazioni, oppure cambia regione con /cambia_regione"
        );

        self.send_text(&text).await
    }
}
