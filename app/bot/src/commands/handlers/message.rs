use super::regions::{ensure_region_selected, stations_scan_page_size};
use crate::commands::context::ChatContext;
use crate::commands::utils;
use crate::station;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use teloxide::prelude::Bot;
use teloxide::types::Message;

pub(crate) async fn message_handler(
    bot: &Bot,
    msg: &Message,
    dynamodb_client: &DynamoDbClient,
) -> Result<(), teloxide::RequestError> {
    let link_preview_options = utils::link_preview_small_media();
    let ctx = ChatContext::from_message(dynamodb_client, msg);
    let Some(text) = msg.text() else {
        return Ok(());
    };

    let Some(stations_table_name) =
        ensure_region_selected(&ctx, bot, msg, link_preview_options.clone()).await?
    else {
        return Ok(());
    };
    let scan_page_size = stations_scan_page_size();
    let station_query = text.trim().replace("@erfiume_bot", "").replace("/", "");
    let text = match station::search::get_station_with_match(
        dynamodb_client,
        station_query,
        stations_table_name.as_str(),
        scan_page_size,
    )
    .await
    {
        Ok(Some((item, match_kind))) => {
            let mut message = item.create_station_message().to_string();
            if matches!(match_kind, station::search::StationMatch::Fuzzy) {
                message.push_str(
                    "\nSe non è la stazione corretta prova ad affinare la ricerca.",
                );
            }
            message
        }
        Err(_) | Ok(None) => "Nessuna stazione trovata con la parola di ricerca.\nInserisci esattamente il nome che vedi nella pagina della regione selezionata:\n- Emilia-Romagna: https://allertameteo.regione.emilia-romagna.it/livello-idrometrico\n- Marche: http://app.protezionecivile.marche.it/sol/annaliidro2/index.sol?lang=it\nSe non sai quale cercare, prova con /stazioni oppure cambia regione con /cambia_regione.".to_string(),
    };

    let mut message = text.clone();
    if fastrand::usize(0..10) == 8 {
        message = format!(
            "{text}\n\nContribuisci al progetto per mantenerlo attivo e sviluppare nuove funzionalità tramite una donazione: https://buymeacoffee.com/d0d0",
        );
    }
    if fastrand::usize(0..50) == 8 {
        message = format!(
            "{text}\n\nEsplora o contribuisci al progetto open-source per sviluppare nuove funzionalità: https://github.com/notdodo/erfiume_bot"
        );
    }
    utils::send_message(bot, msg, link_preview_options, &message).await?;

    Ok(())
}
