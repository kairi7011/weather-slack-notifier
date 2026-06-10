use crate::{
    config::Config,
    error::{AppError, Result},
};
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct SlackResponse {
    ok: bool,
    error: Option<String>,
}

pub async fn post_to_slack(client: &Client, config: &Config, text: &str) -> Result<()> {
    let response = client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(&config.slack_bot_token)
        .json(&serde_json::json!({
            "channel": config.slack_channel_id,
            "text": text,
        }))
        .send()
        .await?
        .error_for_status()?;

    let body: SlackResponse = response.json().await?;
    if body.ok {
        return Ok(());
    }

    Err(AppError::new(format!(
        "slack API error: {}",
        body.error.unwrap_or_else(|| "unknown".to_string())
    )))
}
