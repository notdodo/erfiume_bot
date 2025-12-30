use crate::logging;
use anyhow::Error;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use chrono::Utc;
use erfiume_dynamodb::chats as dynamo_chats;
use teloxide::types::Message;
use tokio::sync::Mutex;

#[derive(Debug)]
pub(crate) enum ChatRegionLookupError {
    MissingChatsTable,
    LookupFailed(Error),
}

impl ChatRegionLookupError {
    pub(crate) fn user_message(&self) -> &'static str {
        match self {
            ChatRegionLookupError::MissingChatsTable => {
                "Configurazione non disponibile. Riprova più tardi."
            }
            ChatRegionLookupError::LookupFailed(_) => {
                "Errore nel recupero della regione. Riprova più tardi."
            }
        }
    }
}

#[derive(Default)]
struct RegionKeyCache {
    loaded: bool,
    value: Option<String>,
}

#[derive(Clone)]
struct ChatPresenceData {
    username: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    title: Option<String>,
}
pub(crate) struct ChatContext {
    dynamodb_client: DynamoDbClient,
    chat_id: i64,
    chat_type: &'static str,
    chats_table_name: Option<String>,
    region_key: Mutex<RegionKeyCache>,
    presence_data: Option<ChatPresenceData>,
    presence_written: Mutex<bool>,
}

impl ChatContext {
    pub(crate) fn from_message(dynamodb_client: &DynamoDbClient, msg: &Message) -> Self {
        Self::new(
            dynamodb_client.clone(),
            msg.chat.id.0,
            chat_type_name(msg),
            Some(ChatPresenceData {
                username: msg.chat.username().map(|value| value.to_string()),
                first_name: msg.chat.first_name().map(|value| value.to_string()),
                last_name: msg.chat.last_name().map(|value| value.to_string()),
                title: msg.chat.title().map(|value| value.to_string()),
            }),
        )
    }

    pub(crate) fn from_chat_id(dynamodb_client: &DynamoDbClient, chat_id: i64) -> Self {
        Self::new(dynamodb_client.clone(), chat_id, "unknown", None)
    }

    fn new(
        dynamodb_client: DynamoDbClient,
        chat_id: i64,
        chat_type: &'static str,
        presence_data: Option<ChatPresenceData>,
    ) -> Self {
        Self {
            dynamodb_client,
            chat_id,
            chat_type,
            chats_table_name: load_chats_table_name(),
            region_key: Mutex::new(RegionKeyCache::default()),
            presence_data,
            presence_written: Mutex::new(false),
        }
    }

    pub(crate) fn dynamodb_client(&self) -> &DynamoDbClient {
        &self.dynamodb_client
    }

    pub(crate) fn chats_table_name(&self) -> Option<&str> {
        self.chats_table_name.as_deref()
    }

    pub(crate) async fn region_key(&self) -> Result<Option<String>, ChatRegionLookupError> {
        {
            let cache = self.region_key.lock().await;
            if cache.loaded {
                return Ok(cache.value.clone());
            }
        }

        let Some(table_name) = self.chats_table_name.as_deref() else {
            return Err(ChatRegionLookupError::MissingChatsTable);
        };

        let result =
            dynamo_chats::get_chat_region(&self.dynamodb_client, table_name, self.chat_id).await;
        let mut cache = self.region_key.lock().await;
        if cache.loaded {
            return Ok(cache.value.clone());
        }

        match result {
            Ok(value) => {
                cache.loaded = true;
                cache.value = value.clone();
                Ok(value)
            }
            Err(err) => Err(ChatRegionLookupError::LookupFailed(err)),
        }
    }

    pub(crate) async fn region_key_with_logging(
        &self,
        logger: &logging::Logger,
    ) -> Result<Option<String>, ChatRegionLookupError> {
        self.ensure_chat_presence_with_logging(logger).await;
        match self.region_key().await {
            Ok(value) => Ok(value),
            Err(ChatRegionLookupError::MissingChatsTable) => {
                let err = "Missing env var: CHATS_TABLE_NAME";
                logger.error("chats.config_missing", &err, "Missing chat configuration");
                Err(ChatRegionLookupError::MissingChatsTable)
            }
            Err(ChatRegionLookupError::LookupFailed(err)) => {
                logger.error(
                    "chats.region_lookup_failed",
                    &err,
                    "Failed to load chat region",
                );
                Err(ChatRegionLookupError::LookupFailed(err))
            }
        }
    }

    pub(crate) async fn ensure_chat_presence_with_logging(&self, logger: &logging::Logger) {
        let Some(chats_table_name) = self.chats_table_name.as_deref() else {
            return;
        };
        let Some(presence_data) = self.presence_data.clone() else {
            return;
        };

        {
            let mut written = self.presence_written.lock().await;
            if *written {
                return;
            }
            *written = true;
        }

        let record = dynamo_chats::ChatRecord {
            chat_id: self.chat_id,
            chat_type: self.chat_type.to_string(),
            username: presence_data.username,
            first_name: presence_data.first_name,
            last_name: presence_data.last_name,
            title: presence_data.title,
            region: None,
            created_at: Utc::now().timestamp(),
        };

        if let Err(err) =
            dynamo_chats::insert_chat_if_missing(&self.dynamodb_client, chats_table_name, &record)
                .await
        {
            logger.clone().table(chats_table_name).error(
                "chats.insert_failed",
                &err,
                "Failed to store chat",
            );
        }
    }
}

fn load_chats_table_name() -> Option<String> {
    let value = std::env::var("CHATS_TABLE_NAME").unwrap_or_default();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn chat_type_name(msg: &Message) -> &'static str {
    if msg.chat.is_private() {
        "private"
    } else if msg.chat.is_group() {
        "group"
    } else if msg.chat.is_supergroup() {
        "supergroup"
    } else if msg.chat.is_channel() {
        "channel"
    } else {
        "other"
    }
}
