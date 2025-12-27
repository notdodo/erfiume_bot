use anyhow::{Result, anyhow};
use aws_sdk_dynamodb::{
    Client, error::SdkError, operation::put_item::PutItemError, types::AttributeValue,
};
use std::collections::HashMap;

pub struct ChatRecord {
    pub chat_id: i64,
    pub chat_type: String,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub title: Option<String>,
    pub created_at: i64,
}

pub async fn insert_chat_if_missing(
    client: &Client,
    table_name: &str,
    record: &ChatRecord,
) -> Result<bool> {
    if table_name.is_empty() {
        return Err(anyhow!("chats table name is empty"));
    }

    let mut item = HashMap::from([
        (
            "chat_id".to_string(),
            AttributeValue::N(record.chat_id.to_string()),
        ),
        (
            "chat_type".to_string(),
            AttributeValue::S(record.chat_type.clone()),
        ),
        (
            "created_at".to_string(),
            AttributeValue::N(record.created_at.to_string()),
        ),
    ]);

    if let Some(username) = record.username.as_ref().filter(|value| !value.is_empty()) {
        item.insert(
            "username".to_string(),
            AttributeValue::S(username.to_string()),
        );
    }
    if let Some(first_name) = record.first_name.as_ref().filter(|value| !value.is_empty()) {
        item.insert(
            "first_name".to_string(),
            AttributeValue::S(first_name.to_string()),
        );
    }
    if let Some(last_name) = record.last_name.as_ref().filter(|value| !value.is_empty()) {
        item.insert(
            "last_name".to_string(),
            AttributeValue::S(last_name.to_string()),
        );
    }
    if let Some(title) = record.title.as_ref().filter(|value| !value.is_empty()) {
        item.insert("title".to_string(), AttributeValue::S(title.to_string()));
    }

    let result = client
        .put_item()
        .table_name(table_name)
        .set_item(Some(item))
        .condition_expression("attribute_not_exists(chat_id)")
        .send()
        .await;

    match result {
        Ok(_) => Ok(true),
        Err(SdkError::ServiceError(err)) => {
            if let PutItemError::ConditionalCheckFailedException(_) = err.err() {
                Ok(false)
            } else {
                Err(anyhow::Error::new(err.into_err()))
            }
        }
        Err(err) => Err(err.into()),
    }
}
