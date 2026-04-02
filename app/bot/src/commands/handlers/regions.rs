use crate::commands::context::ChatContext;
use crate::commands::utils;
use crate::logging;
use erfiume_core::config::{RegionConfig, RegionsConfig};
use std::sync::OnceLock;
use teloxide::prelude::Bot;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, LinkPreviewOptions, Message};

const REGION_CALLBACK_PREFIX: &str = "region:";

pub(crate) fn region_keyboard(regions: &RegionsConfig) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            regions.emilia_romagna.label.as_str(),
            format!(
                "{REGION_CALLBACK_PREFIX}{}",
                regions.emilia_romagna.key.as_str()
            ),
        ),
        InlineKeyboardButton::callback(
            regions.marche.label.as_str(),
            format!("{REGION_CALLBACK_PREFIX}{}", regions.marche.key.as_str()),
        ),
    ]])
}

pub(crate) fn parse_region_callback_data<'a>(
    data: &str,
    regions: &'a RegionsConfig,
) -> Option<&'a RegionConfig> {
    let key = data.strip_prefix(REGION_CALLBACK_PREFIX)?;
    regions.find_by_key(key)
}

pub(crate) async fn load_region_for_chat<'a>(
    ctx: &ChatContext,
    logger: &logging::Logger,
    regions: &'a RegionsConfig,
) -> Result<Option<&'a RegionConfig>, &'static str> {
    let region_logger = if let Some(table) = ctx.chats_table_name() {
        logger.clone().table(table)
    } else {
        logger.clone()
    };
    let Some(region_key) = ctx
        .region_key_with_logging(&region_logger)
        .await
        .map_err(|err| err.user_message())?
    else {
        return Ok(None);
    };

    if let Some(region) = regions.find_by_key(&region_key) {
        Ok(Some(region))
    } else {
        region_logger.info("chats.region_unknown", "Unknown region in chat record");
        Ok(None)
    }
}

pub(crate) async fn ensure_region_selected(
    ctx: &ChatContext,
    bot: &Bot,
    msg: &Message,
    link_preview_options: LinkPreviewOptions,
) -> Result<Option<String>, teloxide::RequestError> {
    let regions = match regions_config() {
        Ok(value) => value,
        Err(err) => {
            logging::Logger::from_message(msg).error(
                "regions.config_missing",
                &err,
                "Missing regions configuration",
            );
            utils::send_message(
                bot,
                msg,
                link_preview_options,
                "Configurazione non disponibile. Riprova più tardi.",
            )
            .await?;
            return Ok(None);
        }
    };

    let logger = logging::Logger::from_message(msg);
    let region = match load_region_for_chat(ctx, &logger, regions).await {
        Ok(region) => region,
        Err(message) => {
            utils::send_message(bot, msg, link_preview_options, message).await?;
            return Ok(None);
        }
    };

    if let Some(region) = region {
        return Ok(Some(region.table_name.clone()));
    }

    let prompt = "Prima di continuare, scegli la regione da monitorare:";
    utils::send_message_with_markup(
        bot,
        msg,
        link_preview_options,
        prompt,
        region_keyboard(regions),
    )
    .await?;
    Ok(None)
}

pub(crate) fn regions_config() -> Result<&'static RegionsConfig, String> {
    static CONFIG: OnceLock<Result<RegionsConfig, String>> = OnceLock::new();
    match CONFIG.get_or_init(RegionsConfig::from_env) {
        Ok(config) => Ok(config),
        Err(err) => Err(err.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_region_callback_data_matches_known_regions() {
        let regions = sample_regions_config();
        assert!(
            parse_region_callback_data("region:emilia-romagna", &regions)
                .is_some_and(|region| region.key == "emilia-romagna")
        );
        assert!(
            parse_region_callback_data("region:marche", &regions)
                .is_some_and(|region| region.key == "marche")
        );
        assert!(parse_region_callback_data("region:unknown", &regions).is_none());
    }

    #[test]
    fn region_from_key_matches_and_unknown_returns_none() {
        let regions = sample_regions_config();
        assert!(
            regions
                .find_by_key("emilia-romagna")
                .is_some_and(|region| region.key == "emilia-romagna")
        );
        assert!(
            regions
                .find_by_key("Marche")
                .is_some_and(|region| region.key == "marche")
        );
        assert!(regions.find_by_key("unknown").is_none());
    }

    fn sample_regions_config() -> RegionsConfig {
        RegionsConfig {
            emilia_romagna: RegionConfig {
                key: "emilia-romagna".to_string(),
                label: "Emilia-Romagna".to_string(),
                table_name: "EmiliaRomagna-Stations".to_string(),
            },
            marche: RegionConfig {
                key: "marche".to_string(),
                label: "Marche".to_string(),
                table_name: "Marche-Stations".to_string(),
            },
        }
    }
}
