pub mod config;
pub mod error;
pub mod message;
pub mod slack;
pub mod weather;

use clap::Parser;
pub use config::{CliArgs, Config};
pub use error::{AppError, Result};
pub use message::compose_message;
pub use slack::post_to_slack;
pub use weather::{fetch_weather, is_weekday_in_timezone};

pub async fn run() -> Result<()> {
    let args = CliArgs::parse();
    let config = Config::from_args(args)?;

    let client = reqwest::Client::new();

    if config.weekday_only && !is_weekday_in_timezone(&config.timezone)? {
        return Ok(());
    }

    let forecast = fetch_weather(&client, &config).await?;
    let message = compose_message(config.weather_name.as_deref(), &forecast);
    post_to_slack(&client, &config, &message).await?;

    Ok(())
}
