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
    pub region: Option<String>,
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
    if let Some(region) = record.region.as_ref().filter(|value| !value.is_empty()) {
        item.insert("region".to_string(), AttributeValue::S(region.to_string()));
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

pub async fn upsert_chat_region(
    client: &Client,
    table_name: &str,
    record: &ChatRecord,
    region: &str,
) -> Result<()> {
    if table_name.is_empty() {
        return Err(anyhow!("chats table name is empty"));
    }
    if region.trim().is_empty() {
        return Err(anyhow!("region is empty"));
    }

    let mut expression_attribute_values = HashMap::from([
        (":region".to_string(), AttributeValue::S(region.to_string())),
        (
            ":chat_type".to_string(),
            AttributeValue::S(record.chat_type.clone()),
        ),
        (
            ":created_at".to_string(),
            AttributeValue::N(record.created_at.to_string()),
        ),
    ]);

    let mut update_expression = String::from(
        "SET #region = :region, \
        chat_type = if_not_exists(chat_type, :chat_type), \
        created_at = if_not_exists(created_at, :created_at)",
    );

    if let Some(username) = record.username.as_ref().filter(|value| !value.is_empty()) {
        update_expression.push_str(", username = if_not_exists(username, :username)");
        expression_attribute_values.insert(
            ":username".to_string(),
            AttributeValue::S(username.to_string()),
        );
    }
    if let Some(first_name) = record.first_name.as_ref().filter(|value| !value.is_empty()) {
        update_expression.push_str(", first_name = if_not_exists(first_name, :first_name)");
        expression_attribute_values.insert(
            ":first_name".to_string(),
            AttributeValue::S(first_name.to_string()),
        );
    }
    if let Some(last_name) = record.last_name.as_ref().filter(|value| !value.is_empty()) {
        update_expression.push_str(", last_name = if_not_exists(last_name, :last_name)");
        expression_attribute_values.insert(
            ":last_name".to_string(),
            AttributeValue::S(last_name.to_string()),
        );
    }
    if let Some(title) = record.title.as_ref().filter(|value| !value.is_empty()) {
        update_expression.push_str(", title = if_not_exists(title, :title)");
        expression_attribute_values
            .insert(":title".to_string(), AttributeValue::S(title.to_string()));
    }

    client
        .update_item()
        .table_name(table_name)
        .key("chat_id", AttributeValue::N(record.chat_id.to_string()))
        .update_expression(update_expression)
        .expression_attribute_names("#region", "region")
        .set_expression_attribute_values(Some(expression_attribute_values))
        .send()
        .await
        .map(|_| ())
        .map_err(|err| err.into())
}

pub async fn get_chat_region(
    client: &Client,
    table_name: &str,
    chat_id: i64,
) -> Result<Option<String>> {
    if table_name.is_empty() {
        return Err(anyhow!("chats table name is empty"));
    }

    let response = client
        .get_item()
        .table_name(table_name)
        .key("chat_id", AttributeValue::N(chat_id.to_string()))
        .send()
        .await?;

    let Some(item) = response.item else {
        return Ok(None);
    };

    match item.get("region") {
        Some(AttributeValue::S(value)) if !value.is_empty() => Ok(Some(value.clone())),
        _ => Ok(None),
    }
}
