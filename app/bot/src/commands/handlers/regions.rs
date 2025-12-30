use crate::commands::context::ChatContext;
use crate::commands::utils;
use crate::logging;
use std::sync::OnceLock;
use teloxide::prelude::Bot;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, LinkPreviewOptions, Message};

const REGION_CALLBACK_PREFIX: &str = "region:";
const DEFAULT_SCAN_PAGE_SIZE: i32 = 25;
const MAX_SCAN_PAGE_SIZE: i32 = 100;

#[derive(Clone)]
pub(crate) struct RegionConfig {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) table_name: String,
}

pub(crate) struct RegionsConfig {
    pub(crate) emilia_romagna: RegionConfig,
    pub(crate) marche: RegionConfig,
}

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
    region_from_key(regions, key)
}

pub(crate) async fn load_region_for_chat<'a>(
    ctx: &ChatContext,
    logger: &logging::Logger,
    regions: &'a RegionsConfig,
) -> Result<Option<&'a RegionConfig>, &'static str> {
    let region_logger = logger_with_table(logger, ctx.chats_table_name());
    let Some(region_key) = ctx
        .region_key_with_logging(&region_logger)
        .await
        .map_err(|err| err.user_message())?
    else {
        return Ok(None);
    };

    if let Some(region) = region_from_key(regions, &region_key) {
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
    match CONFIG.get_or_init(load_regions_config) {
        Ok(config) => Ok(config),
        Err(err) => Err(err.clone()),
    }
}

pub(crate) fn stations_scan_page_size() -> i32 {
    let raw = std::env::var("STATIONS_SCAN_PAGE_SIZE").unwrap_or_default();
    let parsed = raw.trim().parse::<i32>().ok();
    let value = parsed.unwrap_or(DEFAULT_SCAN_PAGE_SIZE);
    value.clamp(1, MAX_SCAN_PAGE_SIZE)
}

fn logger_with_table(logger: &logging::Logger, table: Option<&str>) -> logging::Logger {
    if let Some(table) = table {
        logger.clone().table(table)
    } else {
        logger.clone()
    }
}

fn region_from_key<'a>(regions: &'a RegionsConfig, key: &str) -> Option<&'a RegionConfig> {
    if key.eq_ignore_ascii_case(regions.emilia_romagna.key.as_str()) {
        Some(&regions.emilia_romagna)
    } else if key.eq_ignore_ascii_case(regions.marche.key.as_str()) {
        Some(&regions.marche)
    } else {
        None
    }
}

fn load_regions_config() -> Result<RegionsConfig, String> {
    let emilia_romagna = RegionConfig {
        key: require_env("REGION_EMILIA_ROMAGNA_KEY")?,
        label: require_env("REGION_EMILIA_ROMAGNA_LABEL")?,
        table_name: require_env("EMILIA_ROMAGNA_STATIONS_TABLE_NAME")?,
    };
    let marche = RegionConfig {
        key: require_env("REGION_MARCHE_KEY")?,
        label: require_env("REGION_MARCHE_LABEL")?,
        table_name: require_env("MARCHE_STATIONS_TABLE_NAME")?,
    };
    Ok(RegionsConfig {
        emilia_romagna,
        marche,
    })
}

fn require_env(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Missing env var: {name}"))
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
            region_from_key(&regions, "emilia-romagna")
                .is_some_and(|region| region.key == "emilia-romagna")
        );
        assert!(region_from_key(&regions, "Marche").is_some_and(|region| region.key == "marche"));
        assert!(region_from_key(&regions, "unknown").is_none());
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
